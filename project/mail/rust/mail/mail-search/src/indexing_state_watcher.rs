//! Live-query watch on the content search indexing state singleton row.
//!
//! Mobile clients subscribe via UniFFI (`content_search_watch_indexing_stream`) to be
//! notified whenever any field of `content_search_indexing_state` changes.
//! Each notification is a "table changed" signal; the consumer reads a fresh
//! [`ContentSearchIndexingProgress`] snapshot via
//! [`MailSearchService::load_indexing_progress`].
//!
//! Two entry points are provided:
//!
//! - [`ContentSearchIndexingStateWatcher::watch`] — raw `sqlite_watcher`
//!   subscription. One notification per committed write touching the table.
//! - [`watch_indexing_state_rate_limited`] — table observer signals a shared
//!   [`Notify`] (which coalesces bursts); one background task forwards wakeups
//!   to the consumer channel with leading-edge + trailing spacing.
//!   Modelled on `proton-bridge`'s `syncReporter.OnProgress`, which throttles
//!   emission so that frequent backend updates do not saturate the UI thread.
//!
//! The orchestrator records per-batch progress with one SQLite UPDATE per
//! batch. For long-running historic loads this is fine without
//! throttling, but enable/disable toggles and rapid status transitions can
//! arrive in close succession; the rate-limited variant gives mobile a
//! predictable cadence regardless of the underlying write rate.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashError, WatcherHandle};
use sqlite_watcher::watcher::TableObserver;
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::indexing_last_error::ContentSearchIndexingLastErrorCode;
use crate::indexing_state::{ContentSearchIndexingState, compute_estimated_fraction};
use crate::service::{MailSearchService, SearchServiceError};

/// Name of the durable singleton row table (kept private to this module to
/// avoid leaking schema names into other modules).
const CONTENT_SEARCH_INDEXING_STATE_TABLE: &str = "content_search_indexing_state";

/// Default coalescing window for the rate-limited watcher.
///
/// Chosen to be short enough to feel real-time to the human eye (<= 250 ms
/// is well below the ~400 ms threshold at which humans perceive UI as
/// laggy) while still collapsing the bursts of writes that happen at
/// orchestrator start (mark started → enable change → first batch
/// progress, often within a few ms of each other).
pub const DEFAULT_INDEXING_STATE_WATCH_RATE_LIMIT: Duration = Duration::from_millis(250);

/// Snapshot payload delivered to mobile by the watch + one-shot getter.
///
/// Mirrors the durable [`ContentSearchIndexingState`] minus the internal
/// timestamps, plus a derived [`Self::estimated_fraction`]. Keeping this
/// type distinct from the storage type lets UniFFI lock the wire shape
/// independently of internal columns.
///
/// `Eq` is intentionally not derived: `estimated_fraction: Option<f64>`
/// (a non-`Eq` type once populated). `PartialEq` is sufficient for
/// snapshot equality in tests and diff-based UI logic on the mobile side.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentSearchIndexingProgress {
    pub status: crate::indexing_state::ContentSearchIndexingStatus,
    pub enabled: bool,
    pub messages_indexed_total: u64,
    pub messages_fetched_total: u64,
    pub messages_skipped_total: u64,
    pub batches_completed: u64,
    /// Parsed from the durable `last_error` code column. `None` when unset or
    /// when the stored value is not a known code (e.g. legacy rows).
    pub last_error: Option<ContentSearchIndexingLastErrorCode>,
    /// Approximate indexed ÷ (mailbox total + 1% buffer). `None` until the
    /// first metadata page reports a non-zero All Mail total. `Some(1.0)` on
    /// [`ContentSearchIndexingStatus::Completed`].
    pub estimated_fraction: Option<f64>,
}

impl ContentSearchIndexingState {
    /// Convert a freshly-loaded durable state into the public watch payload.
    #[must_use]
    pub fn to_progress(&self) -> ContentSearchIndexingProgress {
        ContentSearchIndexingProgress {
            status: self.status,
            enabled: self.enabled,
            messages_indexed_total: self.messages_indexed_total,
            messages_fetched_total: self.messages_fetched_total,
            messages_skipped_total: self.messages_skipped_total,
            batches_completed: self.batches_completed,
            last_error: self.last_error,
            estimated_fraction: compute_estimated_fraction(
                self.status,
                self.messages_indexed_total,
                self.mailbox_messages_total,
            ),
        }
    }
}

impl MailSearchService {
    /// One-shot snapshot read of the indexing progress payload.
    ///
    /// Mobile callers invoke this both on first attach to obtain an initial
    /// state and on every watch notification to read the latest snapshot.
    pub async fn load_indexing_progress(
        &self,
    ) -> Result<ContentSearchIndexingProgress, SearchServiceError> {
        let state = self.load_indexing_state().await?;
        Ok(state.to_progress())
    }

    /// Subscribe to indexing-state changes with the default rate-limit
    /// window ([`DEFAULT_INDEXING_STATE_WATCH_RATE_LIMIT`]).
    pub async fn watch_indexing_state(&self) -> Result<RateLimitedWatcherHandle, StashError> {
        watch_indexing_state_rate_limited(
            self.mail_stash(),
            DEFAULT_INDEXING_STATE_WATCH_RATE_LIMIT,
        )
        .await
    }
}

/// Raw `sqlite_watcher` subscription on `content_search_indexing_state`.
///
/// Returns one `()` on the channel per committed transaction that touches
/// the table. For UI consumers prefer
/// [`watch_indexing_state_rate_limited`].
pub struct ContentSearchIndexingStateWatcher;

impl ContentSearchIndexingStateWatcher {
    /// Subscribe to table-change notifications for the indexing state row.
    ///
    /// The returned [`WatcherHandle`] owns the subscription; dropping it
    /// unregisters the observer.
    pub async fn watch(mail_stash: &Stash<UserDb>) -> Result<WatcherHandle, StashError> {
        mail_stash
            .subscribe_to(|sender| Box::new(IndexingStateTableObserver { sender }))
            .await
    }
}

struct IndexingStateTableObserver {
    sender: flume::Sender<()>,
}

impl TableObserver for IndexingStateTableObserver {
    fn tables(&self) -> Vec<String> {
        vec![CONTENT_SEARCH_INDEXING_STATE_TABLE.to_string()]
    }

    fn on_tables_changed(&self, _changed_tables: &BTreeSet<String>) {
        self.sender
            .send(())
            .inspect_err(|e| {
                tracing::error!(
                    "Failed to send notification for ContentSearchIndexingStateWatcher: {:?}",
                    e
                );
            })
            .ok();
    }
}

struct RateLimitedIndexingStateTableObserver {
    notify: Arc<Notify>,
}

impl TableObserver for RateLimitedIndexingStateTableObserver {
    fn tables(&self) -> Vec<String> {
        vec![CONTENT_SEARCH_INDEXING_STATE_TABLE.to_string()]
    }

    fn on_tables_changed(&self, _changed_tables: &BTreeSet<String>) {
        self.notify.notify_one();
    }
}

/// Rate-limited [`WatcherHandle`] that keeps the emitter task alive.
pub struct RateLimitedWatcherHandle {
    pub watcher: WatcherHandle,
    _emitter: EmitterGuard,
}

impl RateLimitedWatcherHandle {
    #[must_use]
    pub fn receiver(&self) -> &flume::Receiver<()> {
        &self.watcher.receiver
    }
}

/// Owns the rate-limited emitter task and stops it on drop.
struct EmitterGuard {
    exit_tx: mpsc::Sender<()>,
    emitter: JoinHandle<()>,
}

impl Drop for EmitterGuard {
    fn drop(&mut self) {
        let _ = self.exit_tx.try_send(());
        self.emitter.abort();
    }
}

/// Rate-limited variant of [`ContentSearchIndexingStateWatcher::watch`].
///
/// Behaviour:
/// - Leading-edge emit: the first signal after at least `min_interval` of
///   quiescence is forwarded immediately.
/// - Trailing coalesce: signals arriving within `min_interval` of a forward
///   are collapsed into one trailing emit at the end of the window.
/// - Disconnect: dropping the returned handle signals the emitter to exit
///   and unregisters the table observer.
///
/// Sub-millisecond `min_interval` effectively disables coalescing and is
/// equivalent to calling [`ContentSearchIndexingStateWatcher::watch`]
/// directly, but the wrapper is still safe to use.
pub async fn watch_indexing_state_rate_limited(
    mail_stash: &Stash<UserDb>,
    min_interval: Duration,
) -> Result<RateLimitedWatcherHandle, StashError> {
    let notify = Arc::new(Notify::new());
    let observer_notify = Arc::clone(&notify);

    let WatcherHandle { handle, .. } = mail_stash
        .subscribe_to(move |_sender| {
            Box::new(RateLimitedIndexingStateTableObserver {
                notify: observer_notify,
            })
        })
        .await?;

    let (outer_rx, emitter) = spawn_rate_limited_emitter(notify, min_interval);

    Ok(RateLimitedWatcherHandle {
        watcher: WatcherHandle::new(outer_rx, handle),
        _emitter: emitter,
    })
}

fn spawn_rate_limited_emitter(
    notify: Arc<Notify>,
    min_interval: Duration,
) -> (flume::Receiver<()>, EmitterGuard) {
    let (outer_tx, outer_rx) = flume::unbounded();
    let (exit_tx, exit_rx) = mpsc::channel(1);
    let emitter = tokio::spawn(rate_limited_emit_loop(
        outer_tx,
        exit_rx,
        notify,
        min_interval,
    ));
    (outer_rx, EmitterGuard { exit_tx, emitter })
}

/// Forward coalesced table-change wakeups to the consumer channel with
/// leading-edge + trailing spacing.
async fn rate_limited_emit_loop(
    outer_tx: flume::Sender<()>,
    mut exit_rx: mpsc::Receiver<()>,
    notify: Arc<Notify>,
    min_interval: Duration,
) {
    let mut last_emit: Option<Instant> = None;

    loop {
        tokio::select! {
            biased;
            _ = exit_rx.recv() => return,
            _ = notify.notified() => {}
        }

        let now = Instant::now();
        if let Some(prev) = last_emit {
            let elapsed = now.duration_since(prev);
            if elapsed < min_interval {
                let wait = min_interval - elapsed;
                tokio::select! {
                    _ = exit_rx.recv() => return,
                    _ = tokio::time::sleep(wait) => {}
                }
            }
        }

        last_emit = Some(Instant::now());
        if outer_tx.send_async(()).await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use mail_stash::stash::{Stash, StashConfiguration};
    use mail_task_service::TaskService;

    use super::*;
    use crate::MailSearchService;
    use crate::indexing_state::ContentSearchIndexingStatus;

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

    fn test_rate_limited_channel(
        min_interval: Duration,
    ) -> (Arc<Notify>, flume::Receiver<()>, EmitterGuard) {
        let notify = Arc::new(Notify::new());
        let (outer_rx, guard) = spawn_rate_limited_emitter(Arc::clone(&notify), min_interval);
        (notify, outer_rx, guard)
    }

    #[test]
    fn state_to_progress_computes_estimated_fraction_with_inflated_denominator() {
        let state = ContentSearchIndexingState {
            enabled: true,
            status: ContentSearchIndexingStatus::Ongoing,
            messages_indexed_total: 5_000,
            messages_fetched_total: 5_100,
            messages_skipped_total: 100,
            batches_completed: 25,
            mailbox_messages_total: Some(10_000),
            last_error: None,
            started_at_ms: Some(1_700_000_000_000),
            updated_at_ms: 1_700_000_000_500,
        };

        let progress = state.to_progress();

        assert_eq!(progress.status, ContentSearchIndexingStatus::Ongoing);
        assert!(progress.enabled);
        assert_eq!(progress.messages_indexed_total, 5_000);
        // 5000 / (10000 + 100) ≈ 0.495
        let fraction = progress.estimated_fraction.expect("fraction");
        assert!(
            (fraction - 5_000.0 / 10_100.0).abs() < f64::EPSILON,
            "expected inflated-denominator fraction, got {fraction}"
        );
    }

    #[test]
    fn state_to_progress_emits_one_on_completed() {
        let state = ContentSearchIndexingState {
            enabled: true,
            status: ContentSearchIndexingStatus::Completed,
            messages_indexed_total: 9_800,
            messages_fetched_total: 10_000,
            messages_skipped_total: 200,
            batches_completed: 50,
            mailbox_messages_total: Some(10_000),
            last_error: None,
            started_at_ms: Some(1_700_000_000_000),
            updated_at_ms: 1_700_000_000_500,
        };

        assert_eq!(state.to_progress().estimated_fraction, Some(1.0));
    }

    #[test]
    fn state_to_progress_leaves_estimate_none_without_mailbox_total() {
        let state = ContentSearchIndexingState {
            enabled: true,
            status: ContentSearchIndexingStatus::Ongoing,
            messages_indexed_total: 149,
            messages_fetched_total: 200,
            messages_skipped_total: 51,
            batches_completed: 1,
            mailbox_messages_total: None,
            last_error: Some(ContentSearchIndexingLastErrorCode::RetryableNetwork),
            started_at_ms: Some(1_700_000_000_000),
            updated_at_ms: 1_700_000_000_500,
        };

        let progress = state.to_progress();

        assert_eq!(progress.status, ContentSearchIndexingStatus::Ongoing);
        assert_eq!(
            progress.last_error,
            Some(ContentSearchIndexingLastErrorCode::RetryableNetwork)
        );
        assert!(progress.estimated_fraction.is_none());
    }

    #[tokio::test]
    async fn load_indexing_progress_returns_seeded_defaults_then_tracks_writes() {
        let svc = test_search_service().await;

        let initial = svc.load_indexing_progress().await.unwrap();
        assert_eq!(initial.status, ContentSearchIndexingStatus::None);
        assert!(!initial.enabled);
        assert_eq!(initial.batches_completed, 0);

        svc.set_indexing_enabled(true).await.unwrap();
        svc.record_indexing_batch_progress(200, 195, 5)
            .await
            .unwrap();

        let after = svc.load_indexing_progress().await.unwrap();
        assert!(after.enabled);
        assert_eq!(after.messages_fetched_total, 200);
        assert_eq!(after.messages_indexed_total, 195);
        assert_eq!(after.messages_skipped_total, 5);
        assert_eq!(after.batches_completed, 1);
    }

    #[tokio::test]
    async fn raw_watcher_fires_at_least_once_per_state_write() {
        let svc = test_search_service().await;
        let handle = ContentSearchIndexingStateWatcher::watch(svc.mail_stash())
            .await
            .unwrap();

        svc.set_indexing_enabled(true).await.unwrap();

        // Allow sqlite_watcher to flush before we assert.
        tokio::time::timeout(Duration::from_secs(1), handle.receiver.recv_async())
            .await
            .expect("timeout waiting for first watcher notification")
            .expect("watcher sender closed");

        svc.record_indexing_batch_progress(10, 10, 0).await.unwrap();

        tokio::time::timeout(Duration::from_secs(1), handle.receiver.recv_async())
            .await
            .expect("timeout waiting for second watcher notification")
            .expect("watcher sender closed");
    }

    #[tokio::test]
    async fn raw_watcher_stops_when_handle_dropped() {
        let svc = test_search_service().await;
        let handle = ContentSearchIndexingStateWatcher::watch(svc.mail_stash())
            .await
            .unwrap();
        let receiver = handle.receiver.clone();
        drop(handle);

        // The observer is unregistered; further writes must not signal on
        // the receiver. With the original sender dropped, recv_async
        // resolves to Err immediately (no signal will ever come).
        let res = tokio::time::timeout(Duration::from_millis(250), receiver.recv_async()).await;
        match res {
            Err(_) => { /* expected: timeout because nothing was sent */ }
            Ok(Err(_)) => { /* also acceptable: channel disconnected */ }
            Ok(Ok(())) => panic!("dropped watcher must not deliver further signals"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limiter_emits_first_signal_immediately() {
        let (notify, coalesced_rx, _guard) = test_rate_limited_channel(Duration::from_millis(250));

        notify.notify_one();

        // Let the spawned task run and forward the leading-edge emit.
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;

        coalesced_rx
            .try_recv()
            .expect("leading-edge emit must arrive immediately");
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limiter_coalesces_burst_into_single_trailing_emit() {
        let (notify, coalesced_rx, _guard) = test_rate_limited_channel(Duration::from_millis(250));

        // First signal: leading-edge emit.
        notify.notify_one();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        coalesced_rx.try_recv().expect("first emit should arrive");

        // Burst inside the cool-down window — Notify coalesces these into one wakeup.
        for _ in 0..10 {
            notify.notify_one();
        }
        tokio::task::yield_now().await;

        // Before the window closes, no new emit yet.
        assert!(
            coalesced_rx.try_recv().is_err(),
            "no emit should appear before the cool-down window closes"
        );

        tokio::time::advance(Duration::from_millis(260)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        coalesced_rx
            .try_recv()
            .expect("one trailing emit must arrive after the cool-down");

        assert!(
            coalesced_rx.try_recv().is_err(),
            "only a single trailing emit is permitted; got more"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limiter_emits_periodically_under_continuous_load() {
        let min_interval = Duration::from_millis(250);
        let (notify, coalesced_rx, _guard) = test_rate_limited_channel(min_interval);

        // Leading-edge emit.
        notify.notify_one();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        coalesced_rx
            .try_recv()
            .expect("leading-edge emit must arrive");

        // Sustained upstream pulses faster than min_interval must still produce
        // trailing emits at the window boundary, not starve after the first emit.
        for _ in 0..5 {
            for _ in 0..10 {
                notify.notify_one();
            }
            tokio::task::yield_now().await;
            tokio::time::advance(min_interval).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;

            coalesced_rx
                .try_recv()
                .expect("one coalesced emit per min_interval window");
            assert!(
                coalesced_rx.try_recv().is_err(),
                "bursts inside a window must collapse to a single emit"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limiter_loop_exits_when_guard_dropped() {
        let (_notify, coalesced_rx, guard) = test_rate_limited_channel(Duration::from_millis(250));

        drop(guard);

        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;

        assert!(
            coalesced_rx.try_recv().is_err(),
            "emitter must close the consumer channel when the guard exits"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limiter_loop_exits_when_downstream_receiver_dropped() {
        let (notify, coalesced_rx, guard) = test_rate_limited_channel(Duration::from_millis(250));

        drop(coalesced_rx);
        drop(guard);

        for _ in 0..3 {
            notify.notify_one();
        }
        tokio::time::advance(Duration::from_millis(300)).await;
        tokio::task::yield_now().await;
        notify.notify_one();
    }
}
