//! Production orchestrator for multi-batch content search historic indexing.
//!
//! Owns the per-session content-search lifecycle: durable status writes, the
//! per-session service slot on [`MailUserContext`] (via
//! [`mail_common::search::ContentSearchHistoricIndexingService`]), the
//! in-process "one run at a time" flag, and translation between walker
//! outcomes and the durable indexing-state row. The multi-batch loop and
//! within-batch metadata pagination live in
//! [`mail_common::historic_mailbox_walker::HistoricMailboxWalker`]; per-batch
//! body fetch + decrypt + index work lives in
//! [`crate::observer::ContentSearchHistoricObserver`].
//!
//! Lifecycle of one invocation of [`ContentSearchIndexingOrchestrator::start`]:
//!
//! 1. Atomically read the durable indexing-state row + the in-process
//!    "is running" flag and feed both into [`StartDecision::from_state`].
//! 2. On `SpawnNew`, reserve the in-process slot, transition the durable row
//!    to `ongoing` via [`MailSearchService::mark_indexing_started_now`], then
//!    spawn the walker on the user context.
//! 3. The spawned task runs [`HistoricMailboxWalker::run`] with the
//!    content-search observer until completion, cancel, observer-driven
//!    stop, or exhaustion of the walker's retry budget. Walker outcome is
//!    then translated to [`IndexingRunOutcome`] and persisted by
//!    [`persist_run_outcome`]. A `Reservation` guard ensures the in-process
//!    slot is released on every exit path, including panic.

use std::sync::Arc;
use std::time::Duration;

use mail_common::MailUserContext;
use mail_common::historic_mailbox_walker::{
    HistoricMailboxWalkOutcome, HistoricMailboxWalker, HistoricMailboxWalkerConfig,
    HistoricMailboxWalkerError,
};
use mail_search::{
    ContentSearchIndexingLastErrorCode, ContentSearchIndexingState, ContentSearchIndexingStatus,
    ContentSearchStartOutcome, MailSearchService, SearchServiceError,
};
use parking_lot::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::EPHEMERAL_HISTORIC_LOAD_BATCH_SIZE;
use crate::continuation::resolve_effective_continuation;
use crate::error::{EphemeralHistoricLoadError, last_error_code_from_incomplete};
use crate::observer::ContentSearchHistoricObserver;

/// Outer cool-down applied between failed orchestrator attempts.
///
/// Matches the proton-bridge `DefaultRetryCoolDown`. The inner resilience
/// wrapper already handles transient network failures with
/// wait-for-connectivity, so this cool-down only kicks in for
/// non-retryable errors that we want to retry a small number of times before
/// surfacing as `Interrupted` to the client.
pub const DEFAULT_RETRY_COOL_DOWN: Duration = Duration::from_secs(20);

/// Maximum number of consecutive failed batch attempts before the loop gives
/// up and persists `Interrupted` with the last error.
pub const DEFAULT_MAX_RETRY_ATTEMPTS: u32 = 3;

/// Default body-fetch concurrency the orchestrator passes through to the
/// inner batch helper when one is not configured explicitly.
pub const DEFAULT_CONCURRENT_BODY_FETCHES: usize = 8;

/// Maximum time to wait for the in-process orchestrator slot to clear before
/// wiping local content-search data ([`Self::cancel_indexing`] with
/// `clear_data: true`).
pub const DEFAULT_CANCEL_WAIT_BEFORE_CLEAR: Duration = Duration::from_secs(5);

/// Knobs that govern the orchestrator's loop behaviour.
///
/// Production paths use [`Self::default`]; tests inject tighter values to
/// keep the cool-down sleep cheap.
#[derive(Debug, Clone)]
pub struct ContentSearchIndexingOrchestratorConfig {
    pub retry_cool_down: Duration,
    pub max_retry_attempts: u32,
    pub concurrent_body_fetches: usize,
}

impl Default for ContentSearchIndexingOrchestratorConfig {
    fn default() -> Self {
        Self {
            retry_cool_down: DEFAULT_RETRY_COOL_DOWN,
            max_retry_attempts: DEFAULT_MAX_RETRY_ATTEMPTS,
            concurrent_body_fetches: DEFAULT_CONCURRENT_BODY_FETCHES,
        }
    }
}

/// Status write performed by [`persist_run_outcome`].
#[derive(Debug)]
pub enum IndexingRunOutcome {
    /// Whole-mailbox historic pass finished naturally with every fetched
    /// message either indexed or persisted as metadata.
    Completed,
    /// Clean cooperative cancel.
    Cancelled,
    /// Walker / observer retry budget exhausted or an unrecoverable failure.
    /// Persist via [`Self::durable_last_error_code`] as a stable code in the
    /// SQLite `last_error` column.
    Fatal {
        error: ContentSearchIndexingLastErrorCode,
    },
    /// A short final batch was processed but some messages were skipped due
    /// to missing body. Surfaced as `Interrupted` with
    /// [`ContentSearchIndexingLastErrorCode::IncompleteWithSkippedBodies`]
    /// so the next start re-runs the loop and gets another opportunity to
    /// drain the tail of the mailbox.
    Incomplete {
        reason: ContentSearchIndexingLastErrorCode,
    },
}

impl PartialEq for IndexingRunOutcome {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Completed, Self::Completed) | (Self::Cancelled, Self::Cancelled) => true,
            (Self::Incomplete { reason: l }, Self::Incomplete { reason: r }) => l == r,
            (Self::Fatal { error: l }, Self::Fatal { error: r }) => l == r,
            _ => false,
        }
    }
}

impl IndexingRunOutcome {
    /// Stable code for durable storage in `last_error` and mobile i18n.
    #[must_use]
    pub fn durable_last_error_code(&self) -> Option<ContentSearchIndexingLastErrorCode> {
        match self {
            Self::Fatal { error } | Self::Incomplete { reason: error } => Some(*error),
            Self::Completed | Self::Cancelled => None,
        }
    }

    fn from_walk_outcome(outcome: HistoricMailboxWalkOutcome) -> Self {
        match outcome {
            HistoricMailboxWalkOutcome::Completed => Self::Completed,
            HistoricMailboxWalkOutcome::Cancelled => Self::Cancelled,
            HistoricMailboxWalkOutcome::Incomplete { reason } => Self::Incomplete {
                reason: last_error_code_from_incomplete(reason),
            },
            HistoricMailboxWalkOutcome::Fatal { error } => Self::Fatal {
                error: last_error_code_from_walker_error(&error),
            },
        }
    }
}

fn last_error_code_from_walker_error(
    error: &HistoricMailboxWalkerError,
) -> ContentSearchIndexingLastErrorCode {
    match error {
        HistoricMailboxWalkerError::Api(_) => ContentSearchIndexingLastErrorCode::FatalApi,
        HistoricMailboxWalkerError::InvalidContinuation => {
            ContentSearchIndexingLastErrorCode::InvalidContinuation
        }
        HistoricMailboxWalkerError::ObserverFatal(err) => err.last_error_code(),
    }
}

/// Pure decision the orchestrator makes at start time, given a snapshot of
/// the durable state and the in-process "is running" flag.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StartDecision {
    NoWork,
    AlreadyRunning,
    SpawnNew,
}

impl StartDecision {
    /// Picks the outcome of `start_indexing` from the durable state row and
    /// the in-process running flag.
    ///
    /// A stale `Ongoing` row (durable says ongoing, in-process flag is false) is
    /// treated the same as `Interrupted` here: the previous run cannot still be
    /// alive, so spawn a fresh one that will resume from the checkpoint.
    #[must_use]
    pub fn from_state(state: &ContentSearchIndexingState, in_process_running: bool) -> Self {
        if in_process_running {
            return Self::AlreadyRunning;
        }
        if !state.enabled {
            return Self::NoWork;
        }
        if state.status == ContentSearchIndexingStatus::Completed {
            return Self::NoWork;
        }
        Self::SpawnNew
    }
}

/// Drive the historic mailbox walker with the content-search observer until
/// natural mailbox end, cancel, observer-driven stop, or exhaustion of the
/// walker's retry budget.
///
/// Resolves the resume anchor from the persisted Foundation Search
/// checkpoint, builds the walker config from
/// [`ContentSearchIndexingOrchestratorConfig`], and translates the walker's
/// terminal outcome into [`IndexingRunOutcome`]. Does **not** transition the
/// orchestrator status field — callers should invoke [`persist_run_outcome`]
/// with the returned outcome.
async fn run_walker_with_observer(
    user_ctx: &Arc<MailUserContext>,
    config: &ContentSearchIndexingOrchestratorConfig,
    cancel: CancellationToken,
) -> IndexingRunOutcome {
    let Some(search_service) = user_ctx.search_service() else {
        return IndexingRunOutcome::Fatal {
            error: ContentSearchIndexingLastErrorCode::FatalApi,
        };
    };
    let resume = match resolve_effective_continuation(search_service, None, true).await {
        Ok(r) => r,
        Err(e) => {
            warn!("ContentSearchIndexingOrchestrator: failed to resolve resume anchor: {e}");
            return IndexingRunOutcome::Fatal {
                error: e.last_error_code(),
            };
        }
    };

    let walker = HistoricMailboxWalker::with_config(HistoricMailboxWalkerConfig {
        batch_size: EPHEMERAL_HISTORIC_LOAD_BATCH_SIZE,
        retry_cool_down: config.retry_cool_down,
        max_retry_attempts: config.max_retry_attempts,
    });
    let observer = ContentSearchHistoricObserver::new(config.concurrent_body_fetches);

    let outcome = walker.run(user_ctx, &observer, resume, cancel).await;
    IndexingRunOutcome::from_walk_outcome(outcome)
}

/// Persist the final orchestrator status based on the loop outcome.
///
/// - `Completed` → `status = completed`, `last_error` cleared.
/// - `Cancelled` → `status = interrupted`, `last_error` cleared (clean stop).
/// - `Fatal` / `Incomplete` → `status = interrupted`, `last_error = stable code`.
pub async fn persist_run_outcome(
    search_service: &MailSearchService,
    outcome: &IndexingRunOutcome,
) -> Result<(), SearchServiceError> {
    match outcome {
        IndexingRunOutcome::Completed => {
            search_service
                .transition_indexing_status(ContentSearchIndexingStatus::Completed, Some(None))
                .await
        }
        IndexingRunOutcome::Cancelled => {
            search_service
                .transition_indexing_status(ContentSearchIndexingStatus::Interrupted, Some(None))
                .await
        }
        IndexingRunOutcome::Fatal { .. } | IndexingRunOutcome::Incomplete { .. } => {
            let last_error_code = outcome.durable_last_error_code().ok_or_else(|| {
                SearchServiceError::IndexingState(
                    "internal error: Fatal/Incomplete outcome missing last_error".into(),
                )
            })?;
            search_service
                .transition_indexing_status(
                    ContentSearchIndexingStatus::Interrupted,
                    Some(Some(last_error_code)),
                )
                .await
        }
    }
}

/// In-process state for the orchestrator (cancel token, abort handle, and
/// the running flag that prevents concurrent runs in the same process).
#[derive(Debug, Default)]
struct OrchestratorInner {
    run_cancel: Option<CancellationToken>,
    task_join: Option<JoinHandle<()>>,
    is_running: bool,
}

struct OrchestratorState {
    inner: RwLock<OrchestratorInner>,
    config: ContentSearchIndexingOrchestratorConfig,
}

/// Per-session orchestrator. At most one run is in flight at a time; calls
/// to [`Self::start`] are idempotent and return one of three outcomes.
///
/// Cheap to clone — internally an `Arc`.
#[derive(Clone)]
pub struct ContentSearchIndexingOrchestrator {
    state: Arc<OrchestratorState>,
}

impl Default for ContentSearchIndexingOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentSearchIndexingOrchestrator {
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ContentSearchIndexingOrchestratorConfig::default())
    }

    #[must_use]
    pub fn with_config(config: ContentSearchIndexingOrchestratorConfig) -> Self {
        Self {
            state: Arc::new(OrchestratorState {
                inner: RwLock::new(OrchestratorInner::default()),
                config,
            }),
        }
    }

    /// Whether a run is currently in flight in this process.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state.inner.read().is_running
    }

    /// Idempotent start.
    ///
    /// Returns immediately for `NoWork` / `AlreadyRunning`; spawns the loop
    /// on `user_ctx` and returns `Started` otherwise.
    pub async fn start(
        &self,
        user_ctx: Arc<MailUserContext>,
    ) -> Result<ContentSearchStartOutcome, EphemeralHistoricLoadError> {
        let search_service = user_ctx
            .search_service()
            .ok_or(EphemeralHistoricLoadError::Other(anyhow::anyhow!(
                "Service not available"
            )))?;

        // Snapshot state under lock, then decide.
        let state = search_service
            .load_indexing_state()
            .await
            .map_err(EphemeralHistoricLoadError::from_indexing_state)?;

        // Reserve the in-process slot atomically alongside the decision.
        let reservation = {
            let mut inner = self.state.inner.write();
            match StartDecision::from_state(&state, inner.is_running) {
                StartDecision::AlreadyRunning => {
                    return Ok(ContentSearchStartOutcome::AlreadyRunning);
                }
                StartDecision::NoWork => return Ok(ContentSearchStartOutcome::NoWork),
                StartDecision::SpawnNew => {
                    inner.is_running = true;
                    Reservation::new(self.state.clone())
                }
            }
        };

        // Install the cancel token before the first async point so
        // `cancel()` calls between reservation and spawn are picked up by
        // the spawned task on its first iteration.
        let cancel = user_ctx.create_child_cancellation_token();
        self.state.inner.write().run_cancel = Some(cancel.clone());

        if let Err(e) = search_service.mark_indexing_started_now().await {
            // `reservation` drops here and releases the in-process slot.
            return Err(EphemeralHistoricLoadError::from_indexing_state(e));
        }

        let task_orchestrator_state = self.state.clone();
        let task_ctx = user_ctx.clone();
        let task_cancel = cancel.clone();

        let join = user_ctx.spawn(async move {
            // Move `reservation` into the task: its `Drop` is what releases
            // the in-process slot on every exit path (completion, cancel,
            // panic, abort).
            let _reservation = reservation;

            let outcome =
                run_walker_with_observer(&task_ctx, &task_orchestrator_state.config, task_cancel)
                    .await;

            if let Err(e) = persist_run_outcome(
                task_ctx
                    .search_service()
                    .expect("Should be set at this point"),
                &outcome,
            )
            .await
            {
                warn!("ContentSearchIndexingOrchestrator: failed to persist final outcome: {e}");
            }
        });

        self.state.inner.write().task_join = Some(join);
        Ok(ContentSearchStartOutcome::Started)
    }

    /// Signal the running task to stop. No-op if idle.
    ///
    /// **Cooperative path (preferred):** fires the cancellation token so the
    /// batch observes cancel at its yield points, the loop returns
    /// [`IndexingRunOutcome::Cancelled`], and [`persist_run_outcome`] writes
    /// `status = interrupted` with `last_error` cleared.
    ///
    /// **Abrupt path (belt-and-suspenders):** also aborts the spawned task
    /// handle so the in-process slot is released even when the future is
    /// stuck between yield points. An abort drops the task before
    /// [`persist_run_outcome`] may run, which can leave the durable row at
    /// `Ongoing` until startup stale-ongoing repair or the next
    /// [`StartDecision::from_state`] (`in_process_running == false` →
    /// `SpawnNew`). That stale row is non-fatal by design. SQLite
    /// transactions in flight roll back on drop; a skipped post-page
    /// `cleanup()` is non-fatal (warning log only).
    ///
    /// The in-process slot is always released by the [`Reservation`] drop
    /// inside the spawned task (normal exit, cooperative cancel, or abort).
    ///
    /// [`cancel_and_wait`] signals cancel and then awaits the spawned task's
    /// [`JoinHandle`]. Callers that must follow cancel with a write that
    /// races the orchestrator (e.g. wiping local index tables) should use
    /// [`cancel_and_wait`], not this method alone.
    pub fn cancel(&self) {
        let mut inner = self.state.inner.write();
        if let Some(cancel) = inner.run_cancel.take() {
            cancel.cancel();
        }
        if let Some(join) = inner.task_join.take() {
            join.abort();
        }
    }

    /// Signal cancel and wait for the spawned task to finish, up to `timeout`.
    ///
    /// Returns `true` if the task's [`JoinHandle`] resolved within the
    /// timeout (or the orchestrator was already idle), `false` on timeout.
    /// The poll fallback (when no join handle is stored) shares the same
    /// deadline as the join await, so callers see one bounded wait, not two.
    ///
    /// This is the entry point callers must use when they intend to
    /// follow cancel with an operation that races against the
    /// orchestrator's writes (e.g. wiping local content-search data).
    pub async fn cancel_and_wait(&self, timeout: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;

        // Fast path only: not synchronized with the write lock below. The spawned
        // task may release its Reservation between this read and taking `task_join`,
        // in which case we fall through to `wait_until_not_running` (immediate true).
        if !self.is_running() {
            return true;
        }

        let join = {
            let mut inner = self.state.inner.write();
            if let Some(cancel) = inner.run_cancel.take() {
                cancel.cancel();
            }
            inner.task_join.take()
        };

        let Some(join) = join else {
            // No JoinHandle to await: reservation may already have dropped (benign
            // race) or we are in the brief start() window before task_join is stored.
            return wait_until_not_running(self, deadline).await;
        };

        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return false;
        }
        tokio::time::timeout(remaining, join).await.is_ok()
    }

    /// Persist the user's content-search enable preference.
    ///
    /// Setting `false` cancels any in-flight run without clearing local data.
    /// Setting `true` does not auto-start indexing.
    pub async fn set_enabled(
        &self,
        user_ctx: Arc<MailUserContext>,
        enabled: bool,
    ) -> Result<(), EphemeralHistoricLoadError> {
        if !enabled {
            self.cancel();
        }
        user_ctx
            .search_service()
            .ok_or(EphemeralHistoricLoadError::Other(anyhow::anyhow!(
                "Service not available"
            )))?
            .set_indexing_enabled(enabled)
            .await
            .map_err(EphemeralHistoricLoadError::from_indexing_state)
    }

    /// Cancel any in-flight run. When `clear_data` is true, waits for the
    /// in-process slot to clear (up to [`DEFAULT_CANCEL_WAIT_BEFORE_CLEAR`]),
    /// then wipes every locally-persisted content-search artifact (`enabled`
    /// is preserved).
    pub async fn cancel_indexing(
        &self,
        user_ctx: Arc<MailUserContext>,
        clear_data: bool,
    ) -> Result<(), EphemeralHistoricLoadError> {
        if !clear_data {
            self.cancel();
            return Ok(());
        }

        let cleanly_stopped = self.cancel_and_wait(DEFAULT_CANCEL_WAIT_BEFORE_CLEAR).await;
        if !cleanly_stopped {
            warn!(
                "ContentSearchIndexingOrchestrator::cancel_indexing: did not release in-process slot within timeout; proceeding with clear anyway"
            );
        }

        let task_service = user_ctx.core_context().task_service().task_service_arc();
        user_ctx
            .search_service()
            .ok_or(EphemeralHistoricLoadError::Other(anyhow::anyhow!(
                "Service not available"
            )))?
            .clear_index_tables(task_service)
            .await
            .map_err(EphemeralHistoricLoadError::from_indexing_state)
    }

    /// Wipe locally-persisted content-search data.
    ///
    /// If a run is in flight, cancels it first (same wait bound as
    /// [`Self::cancel_indexing`] with `clear_data: true`) so the wipe does not
    /// race batch persistence. Does not restart indexing afterward.
    pub async fn clear_local_data(
        &self,
        user_ctx: Arc<MailUserContext>,
    ) -> Result<(), EphemeralHistoricLoadError> {
        self.cancel_indexing(user_ctx, true).await
    }
}

/// Poll until the in-process slot clears when [`cancel_and_wait`] has no
/// [`JoinHandle`] to await.
///
/// This is hit transiently when the task finishes between the fast-path
/// `is_running()` read and taking `task_join`, or briefly during [`ContentSearchIndexingOrchestrator::start`]
/// after the reservation is taken but before `task_join` is stored. Usually
/// returns on the first poll; shares the caller's overall deadline with the
/// join-await path so the wait is not bounded twice.
async fn wait_until_not_running(
    orchestrator: &ContentSearchIndexingOrchestrator,
    deadline: tokio::time::Instant,
) -> bool {
    let poll_interval = orchestrator_cancel_poll_interval();
    loop {
        if !orchestrator.is_running() {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(poll_interval).await;
    }
}

/// Polling interval used by [`wait_until_not_running`].
///
/// Kept short in tests; modest in production because the common case resolves
/// on the first poll after a reservation drop.
fn orchestrator_cancel_poll_interval() -> Duration {
    if cfg!(test) {
        Duration::from_millis(1)
    } else {
        Duration::from_millis(20)
    }
}

/// RAII guard that releases the in-process orchestrator slot on every exit
/// path: normal completion of the spawned task, panic in the loop, or a
/// caller-initiated `abort()` that drops the future mid-flight.
struct Reservation {
    state: Arc<OrchestratorState>,
}

impl Reservation {
    fn new(state: Arc<OrchestratorState>) -> Self {
        Self { state }
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let mut inner = self.state.inner.write();
        inner.run_cancel = None;
        inner.task_join = None;
        inner.is_running = false;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mail_search::{
        ContentSearchIndexingLastErrorCode, ContentSearchIndexingState,
        ContentSearchIndexingStatus, MailSearchService,
    };
    use mail_stash::stash::{Stash, StashConfiguration};
    use mail_task_service::TaskService;

    use super::*;

    // Walker-loop behaviour (multi-batch progression, mailbox-end detection,
    // retry budget, cancellation timing, cool-down) is now owned by
    // `HistoricMailboxWalker` in `mail-common` and is exercised by walker
    // tests in that crate. The orchestrator tests below cover only the
    // lifecycle concerns the orchestrator retains: `StartDecision` from the
    // durable + in-process state, durable status writes via
    // `persist_run_outcome`, and the `cancel_and_wait` join semantics.

    async fn test_search_service() -> MailSearchService {
        let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
        let task_service = Arc::new(
            TaskService::new(tokio::runtime::Handle::current())
                .expect("Failed to create TaskService"),
        );
        MailSearchService::new(mail_stash, task_service)
            .await
            .expect("MailSearchService::new")
    }

    // -- StartDecision::from_state -----------------------------------------

    fn state_with(
        enabled: bool,
        status: ContentSearchIndexingStatus,
    ) -> ContentSearchIndexingState {
        ContentSearchIndexingState {
            enabled,
            status,
            messages_indexed_total: 0,
            messages_fetched_total: 0,
            messages_skipped_total: 0,
            batches_completed: 0,
            mailbox_messages_total: None,
            last_error: None,
            started_at_ms: None,
            updated_at_ms: 0,
        }
    }

    #[test]
    fn evaluate_returns_already_running_when_in_process_flag_is_set() {
        let s = state_with(true, ContentSearchIndexingStatus::Interrupted);
        assert_eq!(
            StartDecision::from_state(&s, true),
            StartDecision::AlreadyRunning
        );
    }

    #[test]
    fn evaluate_returns_no_work_when_disabled() {
        let s = state_with(false, ContentSearchIndexingStatus::Interrupted);
        assert_eq!(StartDecision::from_state(&s, false), StartDecision::NoWork);
    }

    #[test]
    fn evaluate_returns_no_work_when_status_is_completed() {
        let s = state_with(true, ContentSearchIndexingStatus::Completed);
        assert_eq!(StartDecision::from_state(&s, false), StartDecision::NoWork);
    }

    #[test]
    fn evaluate_spawns_for_none_interrupted_and_stale_ongoing() {
        for status in [
            ContentSearchIndexingStatus::None,
            ContentSearchIndexingStatus::Interrupted,
            ContentSearchIndexingStatus::Ongoing, // stale: in-process flag is false
        ] {
            let s = state_with(true, status);
            assert_eq!(
                StartDecision::from_state(&s, false),
                StartDecision::SpawnNew,
                "expected SpawnNew for enabled+{status:?}"
            );
        }
    }

    // -- persist_run_outcome -----------------------------------------------

    #[tokio::test]
    async fn persist_completed_writes_status_completed_and_clears_last_error() {
        let svc = test_search_service().await;
        svc.transition_indexing_status(
            ContentSearchIndexingStatus::Interrupted,
            Some(Some(ContentSearchIndexingLastErrorCode::MetadataPrepare)),
        )
        .await
        .unwrap();

        persist_run_outcome(&svc, &IndexingRunOutcome::Completed)
            .await
            .unwrap();

        let state = svc.load_indexing_state().await.unwrap();
        assert_eq!(state.status, ContentSearchIndexingStatus::Completed);
        assert!(state.last_error.is_none());
    }

    #[tokio::test]
    async fn persist_cancelled_writes_interrupted_with_no_last_error() {
        let svc = test_search_service().await;

        persist_run_outcome(&svc, &IndexingRunOutcome::Cancelled)
            .await
            .unwrap();

        let state = svc.load_indexing_state().await.unwrap();
        assert_eq!(state.status, ContentSearchIndexingStatus::Interrupted);
        assert!(
            state.last_error.is_none(),
            "clean cancel must not populate last_error"
        );
    }

    #[tokio::test]
    async fn persist_incomplete_writes_interrupted_with_stable_error_code() {
        let svc = test_search_service().await;

        persist_run_outcome(
            &svc,
            &IndexingRunOutcome::Incomplete {
                reason: ContentSearchIndexingLastErrorCode::IncompleteWithSkippedBodies,
            },
        )
        .await
        .unwrap();

        let state = svc.load_indexing_state().await.unwrap();
        assert_eq!(state.status, ContentSearchIndexingStatus::Interrupted);
        assert_eq!(
            state.last_error,
            Some(ContentSearchIndexingLastErrorCode::IncompleteWithSkippedBodies)
        );
        let progress = state.to_progress();
        assert_eq!(
            progress.last_error,
            Some(ContentSearchIndexingLastErrorCode::IncompleteWithSkippedBodies)
        );
    }

    #[tokio::test]
    async fn persist_fatal_writes_interrupted_with_stable_error_code() {
        let svc = test_search_service().await;

        persist_run_outcome(
            &svc,
            &IndexingRunOutcome::Fatal {
                error: ContentSearchIndexingLastErrorCode::PagePersist,
            },
        )
        .await
        .unwrap();

        let state = svc.load_indexing_state().await.unwrap();
        assert_eq!(state.status, ContentSearchIndexingStatus::Interrupted);
        assert_eq!(
            state.last_error,
            Some(ContentSearchIndexingLastErrorCode::PagePersist)
        );
    }

    // -- cancel_and_wait ---------------------------------------------------

    #[tokio::test]
    async fn cancel_and_wait_returns_true_immediately_when_orchestrator_is_idle() {
        let orchestrator = ContentSearchIndexingOrchestrator::new();
        assert!(!orchestrator.is_running());

        let start = std::time::Instant::now();
        let result = orchestrator.cancel_and_wait(Duration::from_secs(5)).await;
        assert!(result, "idle orchestrator must report clean stop");
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "idle case must not wait, elapsed = {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn cancel_and_wait_times_out_when_reservation_is_never_released() {
        let orchestrator = ContentSearchIndexingOrchestrator::new();
        // Simulate an in-flight run that never releases its reservation —
        // i.e. the task is stuck (CPU-bound, blocked syscall, etc.). The
        // bound on `cancel_and_wait` must still hold.
        orchestrator.state.inner.write().is_running = true;

        let start = std::time::Instant::now();
        let result = orchestrator
            .cancel_and_wait(Duration::from_millis(20))
            .await;
        let elapsed = start.elapsed();

        assert!(
            !result,
            "stuck reservation must surface as a timeout, not a clean stop"
        );
        assert!(
            elapsed >= Duration::from_millis(20),
            "cancel_and_wait must honour the requested timeout, elapsed = {elapsed:?}"
        );
        orchestrator.state.inner.write().is_running = false;
    }

    #[tokio::test]
    async fn cancel_and_wait_returns_true_when_task_join_completes() {
        let orchestrator = ContentSearchIndexingOrchestrator::new();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
        let join = tokio::spawn(async move {
            release_rx.await.ok();
        });
        {
            let mut inner = orchestrator.state.inner.write();
            inner.is_running = true;
            inner.task_join = Some(join);
        }

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = release_tx.send(());
        });

        let result = orchestrator.cancel_and_wait(Duration::from_secs(2)).await;
        assert!(
            result,
            "cancel_and_wait must resolve when the task join handle completes"
        );
    }
}
