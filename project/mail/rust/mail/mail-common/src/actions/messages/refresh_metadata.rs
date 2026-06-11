use crate::actions::PREFETCH_ROLLBACK_ACTION_GROUP;
use crate::datatypes::LocalMessageId;
use crate::models::{Message, MessageScrollData};
use crate::{MailContextError, MailUserContext};
use itertools::Itertools;
use mail_action_queue::action::{
    Action, ActionGroup, ActionId, DefaultVersionConverter, Handler, Priority, Type,
};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_core_common::models::ModelExtension;
use mail_stash::UserDb;
use mail_stash::stash::WriteTx;
use serde::{self, Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Weak;

/// Refresh message metadata action.
///
/// This action is designed to refresh existing metadata
/// in a case of suspicion that this message may be outdated.
///
/// On failure the local message will be removed.
///
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RefreshMetadata {
    local_ids: Vec<LocalMessageId>,
}

impl RefreshMetadata {
    pub fn new(local_ids: Vec<LocalMessageId>) -> Self {
        Self { local_ids }
    }
}

impl Action<UserDb> for RefreshMetadata {
    const TYPE: Type = Type("refresh_message_metadata");
    const VERSION: u32 = 1;
    const PRIORITY: Priority = Priority::Normal;
    const GROUP: ActionGroup = PREFETCH_ROLLBACK_ACTION_GROUP;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = RefreshMetadataHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailContextError;
}

pub struct RefreshMetadataHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for RefreshMetadataHandler {
    type Action = RefreshMetadata;

    async fn apply_local(
        &self,
        _: ActionId,
        _: &mut Self::Action,
        _: &WriteTx<'_>,
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
        _: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        Ok(())
    }

    async fn apply_remote(
        &self,
        _: ActionId,
        action: &mut Self::Action,
    ) -> Result<
        <Self::Action as Action<UserDb>>::RemoteOutput,
        <Self::Action as Action<UserDb>>::Error,
    > {
        if action.local_ids.is_empty() {
            tracing::debug!("Refresh metadata for messages called with empty id list");
            return Ok(());
        }

        let ctx = self.ctx.upgrade().ok_or(MailContextError::LostContext)?;
        let mut tether = ctx.user_stash().connection();

        let messages = Message::find_by_ids(action.local_ids.clone(), &tether).await?;
        let mut non_drafts = vec![];

        for msg in messages.into_iter().filter(|msg| msg.remote_id.is_some()) {
            if !msg.is_local_draft(&tether).await? {
                non_drafts.push(msg);
            }
        }

        let remote_ids = non_drafts
            .iter()
            .filter_map(|msg| msg.remote_id.clone())
            .collect_vec();

        let items_sync_result = Message::sync_metadata(
            remote_ids.clone(),
            ctx.session(),
            ctx.search_service(),
            &mut tether,
        )
        .await;

        let refreshed_items = match items_sync_result {
            Ok(items) => items,
            Err(MailContextError::Api(e)) if e.is_network_failure() => {
                return Err(MailContextError::Api(e));
            }
            Err(e) => {
                tracing::error!("Unexpected error while refreshing messages metadata: `{e}`");
                tracing::info!("Deleting local messages: `{:?}`", action.local_ids);
                tether
                    .write_tx::<_, _, <Self::Action as Action<UserDb>>::Error>(async |tx| {
                        Message::delete_by_ids(action.local_ids.clone(), tx).await?;
                        MessageScrollData::delete_all(tx).await?;
                        Ok(())
                    })
                    .await?;

                return Err(e);
            }
        };

        let refreshed_ids: HashSet<_> = refreshed_items
            .iter()
            .filter_map(|msg| msg.local_id)
            .collect();

        let not_refreshed = non_drafts
            .iter()
            .filter_map(|msg| msg.local_id)
            .filter(|x| !refreshed_ids.contains(x))
            .collect_vec();

        if !not_refreshed.is_empty() {
            // The conversation appears to be not found remotely, delete it.
            tracing::warn!("Local messages without remote counterpart found while refreshing.");
            tracing::info!("Deleting local messages: `{:?}`", not_refreshed);

            tether
                .write_tx::<_, _, <Self::Action as Action<UserDb>>::Error>(async |tx| {
                    Message::delete_by_ids(not_refreshed, tx).await?;
                    MessageScrollData::delete_all(tx).await?;
                    Ok(())
                })
                .await?;
        }

        Ok(())
    }

    async fn rebase_local(
        &self,
        _: ActionId,
        _: &mut Self::Action,
        _: &RebaseChangeSet,
        _: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        Ok(())
    }
}
