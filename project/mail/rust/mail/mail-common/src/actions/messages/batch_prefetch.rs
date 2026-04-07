//! Batch prefetch action for efficiently storing multiple message bodies in one transaction.
//!
//! Used by historic load, remote search results, and any scenario where multiple
//! messages need bodies. Reduces SQLite write overhead vs per-message Prefetch.

use crate::actions::{BATCH_PREFETCH_ACTION_GROUP, MailActionError};
use crate::datatypes::DeletedItemType;
use crate::datatypes::LocalMessageId;
use crate::models::DeletedItem;
use crate::models::Message;
use crate::models::RawMessageBody;
use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionGroup, ActionId, DefaultVersionConverter, Handler,
    Priority, Type, WriterGuard,
};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api::services::proton::common::MessageId;
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::Bond;
use serde::{self, Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Weak;
use std::time::{Duration, Instant};
#[cfg(feature = "foundation_search_lab_harness")]
use tokio::task::spawn_blocking;
use tracing::debug;
#[cfg(feature = "foundation_search_lab_harness")]
use tracing::error;

/// Maximum number of message bodies to store in a single BatchPrefetch transaction.
/// Tuned for historic load and remote search; larger batches reduce SQLite commit overhead.
/// Kept separate from search worker `MAX_BATCH_SIZE` (Foundation commit cap).
pub const BATCH_PREFETCH_SIZE: usize = 100;

const WRITER_GUARD_REFRESH_INTERVAL: Duration = Duration::from_secs(10);

/// Blocking fixture body lookup (`mail_search_perf`); must run on the blocking pool (`tokio::task::spawn_blocking`).
#[cfg(feature = "foundation_search_lab_harness")]
fn lab_fixture_body_for_batch_prefetch(remote_id: MessageId) -> Option<RawMessageBody> {
    if mail_search_perf::fixture_bodies::is_real_bodies_initialized() {
        match mail_search_perf::fixture_bodies::get_body_for_remote_id(remote_id.as_str()) {
            Ok(real_body) => Some(RawMessageBody::from_fixture(&real_body)),
            Err(e) => {
                error!("Real body error for {}: {}", remote_id, e);
                None
            }
        }
    } else if mail_search_perf::fixture_bodies::is_initialized() {
        match mail_search_perf::fixture_bodies::get_next_body() {
            Ok(fixture_body) => Some(RawMessageBody::from_fixture(&fixture_body)),
            Err(e) => {
                error!("Fixture body error for {}: {}", remote_id, e);
                None
            }
        }
    } else {
        None
    }
}

async fn batch_prefetch_refresh_writer_guard_if_due(
    guard: &mut WriterGuard<'_, UserDb>,
    last_refresh: &mut Instant,
) -> Result<(), MailActionError> {
    if last_refresh.elapsed() < WRITER_GUARD_REFRESH_INTERVAL {
        return Ok(());
    }
    guard.tx::<_, _, MailActionError>(async |_| Ok(())).await?;
    debug!("BatchPrefetch: no-op transaction to keep writer guard lease alive");
    *last_refresh = Instant::now();
    Ok(())
}

#[must_use]
pub fn batch_prefetch_can_ingest_bodies() -> bool {
    #[cfg(feature = "foundation_search_lab_harness")]
    {
        mail_search_perf::fixture_bodies::is_real_bodies_initialized()
            || mail_search_perf::fixture_bodies::is_initialized()
    }
    #[cfg(not(feature = "foundation_search_lab_harness"))]
    {
        false
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BatchPrefetch {
    local_ids: Vec<LocalMessageId>,
}

impl BatchPrefetch {
    pub fn new(local_ids: Vec<LocalMessageId>) -> Self {
        Self { local_ids }
    }
}

impl Action<UserDb> for BatchPrefetch {
    const TYPE: Type = Type("batch_prefetch_message");
    const VERSION: u32 = 1;
    const PRIORITY: Priority = Priority::Lowest;
    const GROUP: ActionGroup = BATCH_PREFETCH_ACTION_GROUP;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = BatchPrefetchHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailActionError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        ActionDependencyKeys::default()
    }
}

pub struct BatchPrefetchHandler {
    pub ctx: Weak<crate::MailUserContext>,
}

impl Handler<UserDb> for BatchPrefetchHandler {
    type Action = BatchPrefetch;

    async fn apply_local(
        &self,
        _: ActionId,
        _: &mut Self::Action,
        _: &Bond<'_>,
    ) -> Result<
        <Self::Action as Action<UserDb>>::LocalOutput,
        <Self::Action as Action<UserDb>>::Error,
    > {
        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        _: &mut Self::Action,
        _: &Bond<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        Ok(())
    }

    async fn apply_remote(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        mut guard: WriterGuard<'_, UserDb>,
    ) -> Result<
        <Self::Action as Action<UserDb>>::RemoteOutput,
        <Self::Action as Action<UserDb>>::Error,
    > {
        let _ctx = self.ctx.upgrade().ok_or(MailActionError::LostContext)?;
        let mut last_guard_refresh = Instant::now();

        let mut candidates: Vec<(LocalMessageId, MessageId)> =
            Vec::with_capacity(action.local_ids.len());

        for local_id in &action.local_ids {
            batch_prefetch_refresh_writer_guard_if_due(&mut guard, &mut last_guard_refresh).await?;

            let Some(local_message) = Message::load(*local_id, guard.tether()).await? else {
                debug!(
                    "Message {} not found for batch prefetch, skipping",
                    local_id
                );
                continue;
            };

            if local_message.deleted {
                debug!("Message {} is deleted, skipping batch prefetch", local_id);
                continue;
            }

            let Some(remote_id) = local_message.remote_id.clone() else {
                debug!(
                    "Message {} has no remote_id, skipping batch prefetch",
                    local_id
                );
                continue;
            };

            candidates.push((*local_id, remote_id));
        }

        let tombstoned: HashSet<String> = if candidates.is_empty() {
            HashSet::new()
        } else {
            batch_prefetch_refresh_writer_guard_if_due(&mut guard, &mut last_guard_refresh).await?;
            DeletedItem::find_deleted_by_remote_ids(
                candidates.iter().map(|(_, rid)| rid.as_str()),
                DeletedItemType::Message,
                guard.tether(),
            )
            .await?
        };

        let mut items: Vec<(LocalMessageId, RawMessageBody)> = Vec::with_capacity(candidates.len());

        for (local_id, remote_id) in candidates {
            batch_prefetch_refresh_writer_guard_if_due(&mut guard, &mut last_guard_refresh).await?;

            if tombstoned.contains(remote_id.as_str()) {
                debug!(
                    "Message {} in deleted_items, skipping batch prefetch",
                    local_id
                );
                continue;
            }

            #[cfg(feature = "foundation_search_lab_harness")]
            let raw = {
                let remote_id = remote_id.clone();
                spawn_blocking(move || lab_fixture_body_for_batch_prefetch(remote_id))
                    .await
                    .map_err(|e| {
                        MailActionError::Other(anyhow::anyhow!(
                            "batch_prefetch lab fixture spawn_blocking: {e}"
                        ))
                    })?
            };

            #[cfg(not(feature = "foundation_search_lab_harness"))]
            let raw: Option<RawMessageBody> = None;

            if let Some(raw_body) = raw {
                items.push((local_id, raw_body));
            }
        }

        if items.is_empty() {
            debug!(
                "BatchPrefetch: no bodies to store (ingest inactive, no fixture match, or skip path)"
            );
            return Ok(());
        }

        let stored_count = items.len();

        guard
            .tx::<_, (), MailActionError>(async |bond| {
                RawMessageBody::store_and_consume_batch(items, bond).await?;
                Ok(())
            })
            .await?;

        debug!(
            "BatchPrefetch: stored {} bodies in one transaction",
            stored_count
        );

        Ok(())
    }

    async fn rebase_local(
        &self,
        _: ActionId,
        _: &mut Self::Action,
        _: &RebaseChangeSet,
        _: &Bond<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        Ok(())
    }
}
