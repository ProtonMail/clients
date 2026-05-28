//! SQLite persistence for the content search indexing state (singleton row).
//!
//! Backs the production `content_search_*` UniFFI surface.
//!
//! The table is a single row holding the user's `enabled` preference, the
//! orchestrator `status`, cumulative counters, and diagnostic fields.
//!
//! The migration seeds a default row at id = 1 so reads always succeed.

use mail_stash::UserDb;
use mail_stash::rusqlite::types::{
    FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef,
};
use mail_stash::stash::{Stash, StashError, WriteTx};

use crate::indexing_last_error::ContentSearchIndexingLastErrorCode;
use crate::service::{MailSearchService, SearchServiceError};

/// Fixed primary key for the singleton indexing-state row.
const INDEXING_STATE_ROW_ID: i64 = 1;

/// Stable code placed in `last_error` when startup recovers a stale `Ongoing` row.
pub(crate) const STALE_ONGOING_RECOVERY_CODE: ContentSearchIndexingLastErrorCode =
    ContentSearchIndexingLastErrorCode::StaleOngoingRecovered;

/// Lifecycle of the historic indexing orchestrator (durable across restarts).
///
/// A distinct `Failed` variant is intentionally not modelled — non-clean
/// exits land in [`Self::Interrupted`] with `last_error` populated.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ContentSearchIndexingStatus {
    None,
    Ongoing,
    Interrupted,
    Completed,
}

/// Outcome of a content-search historic indexing start request.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ContentSearchStartOutcome {
    /// Indexing is disabled, the mailbox is already fully indexed, or there
    /// is nothing to do.
    NoWork,
    /// A new orchestrator run was spawned (covers fresh starts and resumes).
    Started,
    /// A run is already in flight in this process.
    AlreadyRunning,
}

impl ContentSearchIndexingStatus {
    /// Stable string representation persisted in the `status` column.
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Ongoing => "ongoing",
            Self::Interrupted => "interrupted",
            Self::Completed => "completed",
        }
    }

    fn from_db_str(value: &str) -> Result<Self, SearchServiceError> {
        match value {
            "none" => Ok(Self::None),
            "ongoing" => Ok(Self::Ongoing),
            "interrupted" => Ok(Self::Interrupted),
            "completed" => Ok(Self::Completed),
            other => Err(SearchServiceError::IndexingState(format!(
                "unknown status value in DB: {other}"
            ))),
        }
    }
}

impl ToSql for ContentSearchIndexingStatus {
    fn to_sql(&self) -> mail_stash::rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(
            self.as_db_str().as_bytes(),
        )))
    }
}

impl FromSql for ContentSearchIndexingStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Self::from_db_str(value.as_str()?).map_err(|err| {
            FromSqlError::Other(format!("invalid ContentSearchIndexingStatus: {err}").into())
        })
    }
}

/// In-memory snapshot of the singleton indexing-state row.
///
/// All timestamps are Unix epoch milliseconds (matches the migration default
/// `strftime('%s','now') * 1000`).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContentSearchIndexingState {
    pub enabled: bool,
    pub status: ContentSearchIndexingStatus,
    pub messages_indexed_total: u64,
    pub messages_fetched_total: u64,
    pub messages_skipped_total: u64,
    pub batches_completed: u64,
    /// All Mail total from the first metadata response of the pass (`None`
    /// until captured). Not refreshed on resume — cleared with progress reset.
    pub mailbox_messages_total: Option<u64>,
    /// Stable error code persisted for mobile i18n (`last_error` column).
    pub last_error: Option<ContentSearchIndexingLastErrorCode>,
    pub started_at_ms: Option<u64>,
    pub updated_at_ms: u64,
}

/// Compute [`ContentSearchIndexingProgress::estimated_fraction`] from durable
/// counters.
///
/// Uses `messages_indexed_total` over a denominator inflated by 1% (at least
/// +1 message) so the bar approaches but does not reach 100% until
/// [`ContentSearchIndexingStatus::Completed`].
#[must_use]
pub fn compute_estimated_fraction(
    status: ContentSearchIndexingStatus,
    messages_indexed_total: u64,
    mailbox_messages_total: Option<u64>,
) -> Option<f64> {
    if status == ContentSearchIndexingStatus::Completed {
        return Some(1.0);
    }
    let total = mailbox_messages_total?;
    if total == 0 {
        return None;
    }
    let buffer = (total / 100).max(1);
    let effective_total = total.saturating_add(buffer);
    Some((messages_indexed_total as f64 / effective_total as f64).min(1.0))
}

impl MailSearchService {
    /// Read the singleton indexing-state row.
    ///
    /// Always returns a row (the migration seeds a default at id = 1).
    pub async fn load_indexing_state(
        &self,
    ) -> Result<ContentSearchIndexingState, SearchServiceError> {
        load_indexing_state(self.mail_stash()).await
    }

    /// Persist the user's content-search enable preference.
    ///
    /// Does **not** trigger indexing — flipping this only updates the row.
    /// The orchestrator is started explicitly via its own entry point.
    pub async fn set_indexing_enabled(&self, enabled: bool) -> Result<(), SearchServiceError> {
        let mut tether = self.mail_stash().connection();
        let updated_at = now_ms();
        tether
            .write_tx::<_, (), StashError>(async move |bond| {
                bond.execute(
                    "UPDATE content_search_indexing_state
                     SET enabled = ?1, updated_at = ?2
                     WHERE id = ?3",
                    mail_stash::params![enabled as i64, updated_at, INDEXING_STATE_ROW_ID],
                )
                .await?;
                Ok(())
            })
            .await
            .map_err(map_stash_err)
    }

    /// Update `status` (and optional `last_error`) atomically.
    ///
    /// `last_error` semantics:
    /// - `Some(Some(code))` — set to stable error code.
    /// - `Some(None)`      — clear (NULL).
    /// - `None`            — leave existing value untouched.
    pub async fn transition_indexing_status(
        &self,
        status: ContentSearchIndexingStatus,
        last_error: Option<Option<ContentSearchIndexingLastErrorCode>>,
    ) -> Result<(), SearchServiceError> {
        let updated_at = now_ms();
        let mut tether = self.mail_stash().connection();
        tether
            .write_tx::<_, (), StashError>(async move |bond| {
                match last_error {
                    Some(new_last_error) => bond
                        .execute(
                            "UPDATE content_search_indexing_state
                             SET status = ?1, last_error = ?2, updated_at = ?3
                             WHERE id = ?4",
                            mail_stash::params![
                                status,
                                new_last_error,
                                updated_at,
                                INDEXING_STATE_ROW_ID
                            ],
                        )
                        .await
                        .map(|_| ()),
                    None => bond
                        .execute(
                            "UPDATE content_search_indexing_state
                             SET status = ?1, updated_at = ?2
                             WHERE id = ?3",
                            mail_stash::params![status, updated_at, INDEXING_STATE_ROW_ID],
                        )
                        .await
                        .map(|_| ()),
                }?;
                Ok(())
            })
            .await
            .map_err(map_stash_err)
    }

    /// Mark the orchestrator as running: `status = ongoing`, clear
    /// `last_error`, set `started_at` to now.
    pub async fn mark_indexing_started_now(&self) -> Result<(), SearchServiceError> {
        let now = now_ms();
        let mut tether = self.mail_stash().connection();
        tether
            .write_tx::<_, (), StashError>(async move |bond| {
                bond.execute(
                    "UPDATE content_search_indexing_state
                     SET status = ?1,
                         last_error = NULL,
                         started_at = ?2,
                         updated_at = ?3
                     WHERE id = ?4",
                    mail_stash::params![
                        ContentSearchIndexingStatus::Ongoing,
                        now,
                        now,
                        INDEXING_STATE_ROW_ID
                    ],
                )
                .await?;
                Ok(())
            })
            .await
            .map_err(map_stash_err)
    }

    /// Persist the All Mail total from the first metadata page when not yet set.
    ///
    /// Idempotent: later batches and resumed runs leave an existing value
    /// untouched until [`Self::reset_indexing_state`] / clear.
    pub async fn set_mailbox_messages_total_if_unset(
        &self,
        total: u64,
    ) -> Result<(), SearchServiceError> {
        if total == 0 {
            return Ok(());
        }
        let updated_at = now_ms();
        let mut tether = self.mail_stash().connection();
        tether
            .write_tx::<_, (), StashError>(async move |bond| {
                bond.execute(
                    "UPDATE content_search_indexing_state
                     SET mailbox_messages_total = ?1, updated_at = ?2
                     WHERE id = ?3 AND mailbox_messages_total IS NULL",
                    mail_stash::params![total, updated_at, INDEXING_STATE_ROW_ID],
                )
                .await?;
                Ok(())
            })
            .await
            .map_err(map_stash_err)
    }

    /// Add a batch's outcome to the cumulative counters.
    pub async fn record_indexing_batch_progress(
        &self,
        fetched: u64,
        indexed: u64,
        skipped: u64,
    ) -> Result<(), SearchServiceError> {
        let updated_at = now_ms();
        let mut tether = self.mail_stash().connection();
        tether
            .write_tx::<_, (), StashError>(async move |bond| {
                bond.execute(
                    "UPDATE content_search_indexing_state
                     SET messages_fetched_total = messages_fetched_total + ?1,
                         messages_indexed_total = messages_indexed_total + ?2,
                         messages_skipped_total = messages_skipped_total + ?3,
                         batches_completed      = batches_completed + 1,
                         updated_at             = ?4
                     WHERE id = ?5",
                    mail_stash::params![
                        fetched,
                        indexed,
                        skipped,
                        updated_at,
                        INDEXING_STATE_ROW_ID
                    ],
                )
                .await?;
                Ok(())
            })
            .await
            .map_err(map_stash_err)
    }

    /// Reset progress fields to their defaults.
    ///
    /// Preserves `enabled` on purpose: clearing local content-search data
    /// should not silently turn the feature off.
    pub async fn reset_indexing_state(&self) -> Result<(), SearchServiceError> {
        let mut tether = self.mail_stash().connection();
        tether
            .write_tx::<_, (), StashError>(async |bond| {
                reset_indexing_state_in_write_tx(bond)
                    .await
                    .map_err(|e| StashError::Custom(anyhow::anyhow!("{e}")))?;
                Ok(())
            })
            .await
            .map_err(map_stash_err)
    }
}

/// Repair a stale `Ongoing` status persisted by a previous process.
///
/// Called once during [`crate::service::MailSearchService::new`] after
/// migrations. The orchestrator only writes `Ongoing` while it is actively
/// running; if the row is `Ongoing` at startup the previous process must
/// have crashed or been killed. We atomically transition to `Interrupted`
/// with a stable `last_error` code so the next `start_indexing` resumes
/// from the existing checkpoint instead of seeing a half-true state.
///
/// Idempotent — leaves `None` / `Interrupted` / `Completed` untouched.
/// The `started_at` column is preserved on purpose so observers can see
/// how long the previous run was active before crashing.
///
/// The `AND status = 'ongoing'` guard in the UPDATE statement makes this
/// safe against any (currently impossible) race between the read and write.
pub(crate) async fn repair_stale_ongoing_at_startup(
    mail_stash: &Stash<UserDb>,
) -> Result<(), SearchServiceError> {
    let state = load_indexing_state(mail_stash).await?;
    if state.status != ContentSearchIndexingStatus::Ongoing {
        return Ok(());
    }

    let now = now_ms();
    let mut tether = mail_stash.connection();
    tether
        .write_tx::<_, (), StashError>(async move |bond| {
            bond.execute(
                "UPDATE content_search_indexing_state
                 SET status = ?1, last_error = ?2, updated_at = ?3
                 WHERE id = ?4 AND status = 'ongoing'",
                mail_stash::params![
                    ContentSearchIndexingStatus::Interrupted,
                    STALE_ONGOING_RECOVERY_CODE,
                    now,
                    INDEXING_STATE_ROW_ID
                ],
            )
            .await?;
            Ok(())
        })
        .await
        .map_err(map_stash_err)?;

    tracing::info!(
        "Repaired stale ongoing content search indexing status to interrupted on startup"
    );
    Ok(())
}

/// Atomically clear every locally-persisted content search artifact inside a
/// caller-supplied write transaction:
///
/// - `search_index_blobs`
/// - `search_index_content_hashes`
/// - `search_index_intents`
/// - `ephemeral_historic_load_checkpoint`
/// - resets `content_search_indexing_state` (preserves `enabled`)
///
/// This is the canonical "clear local content search data" composition; it
/// is reused by the public `MailSearchService::clear_index_tables` and by
/// the orchestrator's clear path.
///
/// `enabled` and ephemeral metadata rows are intentionally **not** cleared:
/// wiping local content-search data must not silently disable the feature.
pub async fn clear_content_search_local_data_in_write_tx(
    bond: &WriteTx<'_>,
) -> Result<(), SearchServiceError> {
    bond.execute("DELETE FROM search_index_blobs", mail_stash::params![])
        .await
        .map_err(map_stash_err)?;
    bond.execute(
        "DELETE FROM search_index_content_hashes",
        mail_stash::params![],
    )
    .await
    .map_err(map_stash_err)?;
    bond.execute("DELETE FROM search_index_intents", mail_stash::params![])
        .await
        .map_err(map_stash_err)?;
    bond.execute(
        "DELETE FROM ephemeral_historic_load_checkpoint",
        mail_stash::params![],
    )
    .await
    .map_err(map_stash_err)?;
    reset_indexing_state_in_write_tx(bond).await?;
    Ok(())
}

/// Increment cumulative indexing-state counters inside an open write transaction.
///
/// Used by historic-load ACID page persist so batch progress and checkpoint
/// commit atomically. Optionally sets `mailbox_messages_total` when not yet
/// populated.
pub async fn apply_indexing_batch_progress_in_write_tx(
    bond: &WriteTx<'_>,
    fetched: u64,
    indexed: u64,
    skipped: u64,
    mailbox_messages_total: Option<u64>,
) -> Result<(), SearchServiceError> {
    let updated_at = now_ms();
    bond.execute(
        "UPDATE content_search_indexing_state
         SET messages_fetched_total = messages_fetched_total + ?1,
             messages_indexed_total = messages_indexed_total + ?2,
             messages_skipped_total = messages_skipped_total + ?3,
             batches_completed      = batches_completed + 1,
             updated_at             = ?4
         WHERE id = ?5",
        mail_stash::params![fetched, indexed, skipped, updated_at, INDEXING_STATE_ROW_ID],
    )
    .await
    .map_err(map_stash_err)?;

    if let Some(total) = mailbox_messages_total.filter(|&t| t > 0) {
        bond.execute(
            "UPDATE content_search_indexing_state
             SET mailbox_messages_total = ?1, updated_at = ?2
             WHERE id = ?3 AND mailbox_messages_total IS NULL",
            mail_stash::params![total, updated_at, INDEXING_STATE_ROW_ID],
        )
        .await
        .map_err(map_stash_err)?;
    }

    Ok(())
}

/// Reset indexing state inside an open Stash write transaction.
///
/// Lets callers (e.g. the orchestrator's clear path) bundle this with other
/// write operations atomically. Preserves `enabled`.
pub async fn reset_indexing_state_in_write_tx(
    bond: &WriteTx<'_>,
) -> Result<(), SearchServiceError> {
    let now = now_ms();
    bond.execute(
        "UPDATE content_search_indexing_state
         SET status                   = ?1,
             messages_indexed_total   = 0,
             messages_fetched_total   = 0,
             messages_skipped_total   = 0,
             batches_completed        = 0,
             mailbox_messages_total   = NULL,
             last_error               = NULL,
             started_at               = NULL,
             updated_at               = ?2
         WHERE id = ?3",
        mail_stash::params![
            ContentSearchIndexingStatus::None,
            now,
            INDEXING_STATE_ROW_ID
        ],
    )
    .await
    .map_err(map_stash_err)?;
    Ok(())
}

async fn load_indexing_state(
    mail_stash: &Stash<UserDb>,
) -> Result<ContentSearchIndexingState, SearchServiceError> {
    let tether = mail_stash.connection();
    let row = tether
        .sync_query(move |conn| {
            conn.query_row(
                "SELECT enabled, status,
                        messages_indexed_total, messages_fetched_total,
                        messages_skipped_total, batches_completed,
                        mailbox_messages_total,
                        last_error, started_at, updated_at
                 FROM content_search_indexing_state
                 WHERE id = ?1",
                [INDEXING_STATE_ROW_ID],
                |row| {
                    let enabled: i64 = row.get(0)?;
                    let status: ContentSearchIndexingStatus = row.get(1)?;
                    let indexed: u64 = row.get(2)?;
                    let fetched: u64 = row.get(3)?;
                    let skipped: u64 = row.get(4)?;
                    let batches: u64 = row.get(5)?;
                    let mailbox_total: Option<u64> = row.get(6)?;
                    let last_error: Option<ContentSearchIndexingLastErrorCode> = row.get(7)?;
                    let started_at: Option<i64> = row.get(8)?;
                    let updated_at: i64 = row.get(9)?;
                    Ok((
                        enabled,
                        status,
                        indexed,
                        fetched,
                        skipped,
                        batches,
                        mailbox_total,
                        last_error,
                        started_at,
                        updated_at,
                    ))
                },
            )
            .map_err(StashError::from)
        })
        .await
        .map_err(map_stash_err)?;

    let (
        enabled,
        status,
        indexed,
        fetched,
        skipped,
        batches,
        mailbox_total,
        last_error,
        started_at,
        updated_at,
    ) = row;

    Ok(ContentSearchIndexingState {
        enabled: enabled != 0,
        status,
        messages_indexed_total: indexed,
        messages_fetched_total: fetched,
        messages_skipped_total: skipped,
        batches_completed: batches,
        mailbox_messages_total: mailbox_total,
        last_error,
        started_at_ms: match started_at {
            Some(v) => Some(timestamp_i64_to_u64(v)?),
            None => None,
        },
        updated_at_ms: timestamp_i64_to_u64(updated_at)?,
    })
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn timestamp_i64_to_u64(value: i64) -> Result<u64, SearchServiceError> {
    u64::try_from(value)
        .map_err(|_| SearchServiceError::IndexingState(format!("timestamp out of range: {value}")))
}

fn map_stash_err(e: StashError) -> SearchServiceError {
    SearchServiceError::IndexingState(e.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mail_stash::stash::{Stash, StashConfiguration};
    use mail_task_service::TaskService;

    use super::*;
    use crate::MailSearchService;

    async fn test_search_service() -> MailSearchService {
        let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
        crate::migrations::run(&mail_stash).await.unwrap();
        let task_service = Arc::new(
            TaskService::new(tokio::runtime::Handle::current())
                .expect("Failed to create TaskService"),
        );
        MailSearchService::new(mail_stash, task_service)
            .await
            .expect("MailSearchService::new")
    }

    #[tokio::test]
    async fn indexing_state_table_exists_after_migration() {
        let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
        crate::migrations::run(&mail_stash).await.unwrap();

        let tether = mail_stash.connection();
        let exists: bool = tether
            .sync_query(|conn| {
                use mail_stash::rusqlite::OptionalExtension;
                conn.query_row(
                    "SELECT 1 FROM sqlite_master
                     WHERE type = 'table' AND name = 'content_search_indexing_state'
                     LIMIT 1",
                    [],
                    |_| Ok(1),
                )
                .optional()
                .map(|opt| opt.is_some())
                .map_err(StashError::from)
            })
            .await
            .unwrap();
        assert!(
            exists,
            "content_search_indexing_state table should exist after migration"
        );
    }

    #[tokio::test]
    async fn migration_seeds_default_singleton_row() {
        let svc = test_search_service().await;
        let state = svc.load_indexing_state().await.unwrap();

        assert!(!state.enabled);
        assert_eq!(state.status, ContentSearchIndexingStatus::None);
        assert_eq!(state.messages_indexed_total, 0);
        assert_eq!(state.messages_fetched_total, 0);
        assert_eq!(state.messages_skipped_total, 0);
        assert_eq!(state.batches_completed, 0);
        assert!(state.last_error.is_none());
        assert!(state.started_at_ms.is_none());
        assert!(state.mailbox_messages_total.is_none());
        assert!(state.updated_at_ms > 0);
    }

    #[test]
    fn compute_estimated_fraction_inflates_denominator_by_one_percent() {
        let fraction =
            compute_estimated_fraction(ContentSearchIndexingStatus::Ongoing, 5_000, Some(10_000))
                .unwrap();
        assert!((fraction - 5_000.0 / 10_100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_estimated_fraction_returns_one_on_completed() {
        assert_eq!(
            compute_estimated_fraction(ContentSearchIndexingStatus::Completed, 9_000, Some(10_000),),
            Some(1.0)
        );
    }

    #[tokio::test]
    async fn set_mailbox_messages_total_if_unset_is_idempotent() {
        let svc = test_search_service().await;

        svc.set_mailbox_messages_total_if_unset(12_345)
            .await
            .unwrap();
        svc.set_mailbox_messages_total_if_unset(99_999)
            .await
            .unwrap();

        let state = svc.load_indexing_state().await.unwrap();
        assert_eq!(state.mailbox_messages_total, Some(12_345));

        let progress = svc.load_indexing_progress().await.unwrap();
        assert_eq!(progress.estimated_fraction, Some(0.0));
    }

    #[tokio::test]
    async fn load_indexing_progress_emits_fraction_after_total_and_batch() {
        let svc = test_search_service().await;

        svc.set_mailbox_messages_total_if_unset(10_000)
            .await
            .unwrap();
        svc.record_indexing_batch_progress(200, 195, 5)
            .await
            .unwrap();

        let progress = svc.load_indexing_progress().await.unwrap();
        let fraction = progress.estimated_fraction.expect("fraction");
        assert!((fraction - 195.0 / 10_100.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn set_indexing_enabled_toggles_preference_and_bumps_updated_at() {
        let svc = test_search_service().await;
        let before = svc.load_indexing_state().await.unwrap();

        svc.set_indexing_enabled(true).await.unwrap();
        let enabled_state = svc.load_indexing_state().await.unwrap();
        assert!(enabled_state.enabled);
        assert!(enabled_state.updated_at_ms >= before.updated_at_ms);

        svc.set_indexing_enabled(false).await.unwrap();
        assert!(!svc.load_indexing_state().await.unwrap().enabled);
    }

    #[tokio::test]
    async fn transition_indexing_status_updates_status_and_last_error_independently() {
        let svc = test_search_service().await;

        svc.transition_indexing_status(ContentSearchIndexingStatus::Ongoing, Some(None))
            .await
            .unwrap();
        let ongoing = svc.load_indexing_state().await.unwrap();
        assert_eq!(ongoing.status, ContentSearchIndexingStatus::Ongoing);
        assert!(ongoing.last_error.is_none());

        svc.transition_indexing_status(
            ContentSearchIndexingStatus::Interrupted,
            Some(Some(ContentSearchIndexingLastErrorCode::RetryableNetwork)),
        )
        .await
        .unwrap();
        let interrupted = svc.load_indexing_state().await.unwrap();
        assert_eq!(interrupted.status, ContentSearchIndexingStatus::Interrupted);
        assert_eq!(
            interrupted.last_error,
            Some(ContentSearchIndexingLastErrorCode::RetryableNetwork)
        );

        svc.transition_indexing_status(ContentSearchIndexingStatus::Completed, None)
            .await
            .unwrap();
        let completed = svc.load_indexing_state().await.unwrap();
        assert_eq!(completed.status, ContentSearchIndexingStatus::Completed);
        assert_eq!(
            completed.last_error,
            Some(ContentSearchIndexingLastErrorCode::RetryableNetwork),
            "passing None for last_error should leave the prior value untouched"
        );
    }

    #[tokio::test]
    async fn mark_indexing_started_now_sets_ongoing_started_at_and_clears_last_error() {
        let svc = test_search_service().await;

        svc.transition_indexing_status(
            ContentSearchIndexingStatus::Interrupted,
            Some(Some(ContentSearchIndexingLastErrorCode::FatalApi)),
        )
        .await
        .unwrap();
        let pre = svc.load_indexing_state().await.unwrap();
        assert_eq!(
            pre.last_error,
            Some(ContentSearchIndexingLastErrorCode::FatalApi)
        );

        svc.mark_indexing_started_now().await.unwrap();
        let started = svc.load_indexing_state().await.unwrap();
        assert_eq!(started.status, ContentSearchIndexingStatus::Ongoing);
        assert!(started.last_error.is_none());
        assert!(started.started_at_ms.is_some());
    }

    #[tokio::test]
    async fn record_indexing_batch_progress_accumulates_counters() {
        let svc = test_search_service().await;

        svc.record_indexing_batch_progress(200, 195, 5)
            .await
            .unwrap();
        svc.record_indexing_batch_progress(180, 180, 0)
            .await
            .unwrap();

        let state = svc.load_indexing_state().await.unwrap();
        assert_eq!(state.messages_fetched_total, 380);
        assert_eq!(state.messages_indexed_total, 375);
        assert_eq!(state.messages_skipped_total, 5);
        assert_eq!(state.batches_completed, 2);
    }

    #[tokio::test]
    async fn reset_indexing_state_zeroes_progress_but_preserves_enabled() {
        let svc = test_search_service().await;

        svc.set_indexing_enabled(true).await.unwrap();
        svc.mark_indexing_started_now().await.unwrap();
        svc.record_indexing_batch_progress(200, 149, 51)
            .await
            .unwrap();
        svc.set_mailbox_messages_total_if_unset(10_000)
            .await
            .unwrap();
        svc.transition_indexing_status(
            ContentSearchIndexingStatus::Interrupted,
            Some(Some(ContentSearchIndexingLastErrorCode::RetryableNetwork)),
        )
        .await
        .unwrap();

        svc.reset_indexing_state().await.unwrap();

        let cleared = svc.load_indexing_state().await.unwrap();
        assert!(
            cleared.enabled,
            "reset must preserve the user's enabled preference"
        );
        assert_eq!(cleared.status, ContentSearchIndexingStatus::None);
        assert_eq!(cleared.messages_indexed_total, 0);
        assert_eq!(cleared.messages_fetched_total, 0);
        assert_eq!(cleared.messages_skipped_total, 0);
        assert_eq!(cleared.batches_completed, 0);
        assert!(cleared.mailbox_messages_total.is_none());
        assert!(cleared.last_error.is_none());
        assert!(cleared.started_at_ms.is_none());
    }

    #[tokio::test]
    async fn ongoing_status_is_repaired_to_interrupted_on_new_service() {
        let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
        crate::migrations::run(&mail_stash).await.unwrap();
        let task_service = Arc::new(
            TaskService::new(tokio::runtime::Handle::current())
                .expect("Failed to create TaskService"),
        );

        let svc1 = MailSearchService::new(mail_stash.clone(), task_service.clone())
            .await
            .unwrap();
        svc1.mark_indexing_started_now().await.unwrap();
        let started_at_ms = svc1
            .load_indexing_state()
            .await
            .unwrap()
            .started_at_ms
            .expect("started_at should be set after mark_indexing_started_now");
        drop(svc1);

        // A fresh service on the same stash must repair the stale `ongoing` row.
        let svc2 = MailSearchService::new(mail_stash, task_service)
            .await
            .unwrap();
        let repaired = svc2.load_indexing_state().await.unwrap();
        assert_eq!(repaired.status, ContentSearchIndexingStatus::Interrupted);
        assert_eq!(repaired.last_error, Some(STALE_ONGOING_RECOVERY_CODE));
        assert_eq!(
            repaired.started_at_ms,
            Some(started_at_ms),
            "started_at must be preserved across the repair so resume diagnostics survive"
        );
    }

    #[tokio::test]
    async fn repair_leaves_interrupted_completed_and_none_states_untouched() {
        for (status, prior_error) in [
            (ContentSearchIndexingStatus::None, None),
            (
                ContentSearchIndexingStatus::Interrupted,
                Some(ContentSearchIndexingLastErrorCode::PagePersist),
            ),
            (ContentSearchIndexingStatus::Completed, None),
        ] {
            let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
            crate::migrations::run(&mail_stash).await.unwrap();
            let task_service = Arc::new(
                TaskService::new(tokio::runtime::Handle::current())
                    .expect("Failed to create TaskService"),
            );

            let svc = MailSearchService::new(mail_stash.clone(), task_service.clone())
                .await
                .unwrap();
            svc.transition_indexing_status(status, Some(prior_error))
                .await
                .unwrap();
            let before = svc.load_indexing_state().await.unwrap();
            drop(svc);

            let _svc2 = MailSearchService::new(mail_stash, task_service)
                .await
                .unwrap();
            let after = _svc2.load_indexing_state().await.unwrap();
            assert_eq!(
                after.status, before.status,
                "status unchanged for {status:?}"
            );
            assert_eq!(
                after.last_error, before.last_error,
                "last_error unchanged for {status:?}"
            );
        }
    }

    #[tokio::test]
    async fn repair_is_idempotent_across_repeated_service_constructions() {
        let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
        crate::migrations::run(&mail_stash).await.unwrap();
        let task_service = Arc::new(
            TaskService::new(tokio::runtime::Handle::current())
                .expect("Failed to create TaskService"),
        );

        let svc = MailSearchService::new(mail_stash.clone(), task_service.clone())
            .await
            .unwrap();
        svc.mark_indexing_started_now().await.unwrap();
        drop(svc);

        let svc2 = MailSearchService::new(mail_stash.clone(), task_service.clone())
            .await
            .unwrap();
        let after_first_repair = svc2.load_indexing_state().await.unwrap();
        assert_eq!(
            after_first_repair.status,
            ContentSearchIndexingStatus::Interrupted
        );
        drop(svc2);

        // A second new service on the now-`interrupted` row must be a no-op
        // (the repair guard short-circuits and does not touch updated_at).
        let svc3 = MailSearchService::new(mail_stash, task_service)
            .await
            .unwrap();
        let after_second_repair = svc3.load_indexing_state().await.unwrap();
        assert_eq!(
            after_second_repair.updated_at_ms, after_first_repair.updated_at_ms,
            "idempotent repair must not bump updated_at"
        );
        assert_eq!(
            after_second_repair.last_error,
            Some(STALE_ONGOING_RECOVERY_CODE),
        );
    }

    #[tokio::test]
    async fn clear_index_tables_resets_indexing_state_and_preserves_enabled() {
        let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
        crate::migrations::run(&mail_stash).await.unwrap();
        let task_service = Arc::new(
            TaskService::new(tokio::runtime::Handle::current())
                .expect("Failed to create TaskService"),
        );
        let svc = MailSearchService::new(mail_stash, task_service.clone())
            .await
            .unwrap();

        svc.set_indexing_enabled(true).await.unwrap();
        svc.mark_indexing_started_now().await.unwrap();
        svc.record_indexing_batch_progress(200, 200, 0)
            .await
            .unwrap();
        svc.transition_indexing_status(
            ContentSearchIndexingStatus::Interrupted,
            Some(Some(ContentSearchIndexingLastErrorCode::IndexPrepare)),
        )
        .await
        .unwrap();

        svc.clear_index_tables(task_service).await.unwrap();

        let cleared = svc.load_indexing_state().await.unwrap();
        assert!(
            cleared.enabled,
            "clearing local data must preserve the user's enabled preference"
        );
        assert_eq!(cleared.status, ContentSearchIndexingStatus::None);
        assert_eq!(cleared.messages_indexed_total, 0);
        assert_eq!(cleared.messages_fetched_total, 0);
        assert_eq!(cleared.messages_skipped_total, 0);
        assert_eq!(cleared.batches_completed, 0);
        assert!(cleared.last_error.is_none());
        assert!(cleared.started_at_ms.is_none());
    }

    #[tokio::test]
    async fn clear_content_search_local_data_in_write_tx_is_atomic_with_other_writes() {
        let svc = test_search_service().await;
        svc.set_indexing_enabled(true).await.unwrap();
        svc.record_indexing_batch_progress(50, 50, 0).await.unwrap();

        let mut tether = svc.mail_stash().connection();
        tether
            .write_tx::<_, (), StashError>(async |bond| {
                clear_content_search_local_data_in_write_tx(bond)
                    .await
                    .map_err(|e| StashError::Custom(anyhow::anyhow!("{e}")))?;
                Ok(())
            })
            .await
            .unwrap();

        let cleared = svc.load_indexing_state().await.unwrap();
        assert!(cleared.enabled, "in-tx clear must preserve enabled");
        assert_eq!(cleared.status, ContentSearchIndexingStatus::None);
        assert_eq!(cleared.batches_completed, 0);

        // Index tables should also be empty after the in-tx clear.
        let tether = svc.mail_stash().connection();
        let blob_count: i64 = tether
            .sync_query(|conn| {
                conn.query_row("SELECT COUNT(*) FROM search_index_blobs", [], |row| {
                    row.get(0)
                })
                .map_err(StashError::from)
            })
            .await
            .unwrap();
        assert_eq!(blob_count, 0);
    }

    #[tokio::test]
    async fn reset_indexing_state_in_write_tx_can_be_composed_with_other_writes() {
        let svc = test_search_service().await;
        svc.set_indexing_enabled(true).await.unwrap();
        svc.record_indexing_batch_progress(200, 200, 0)
            .await
            .unwrap();

        let mut tether = svc.mail_stash().connection();
        tether
            .write_tx::<_, (), StashError>(async |bond| {
                reset_indexing_state_in_write_tx(bond)
                    .await
                    .map_err(|e| StashError::Custom(anyhow::anyhow!("{e}")))?;
                Ok(())
            })
            .await
            .unwrap();

        let cleared = svc.load_indexing_state().await.unwrap();
        assert_eq!(cleared.status, ContentSearchIndexingStatus::None);
        assert_eq!(cleared.batches_completed, 0);
        assert!(cleared.enabled, "enabled preserved across in-tx reset");
    }
}
