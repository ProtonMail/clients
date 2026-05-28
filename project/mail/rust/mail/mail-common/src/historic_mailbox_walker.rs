//! Generic page-by-page walker over All Mail metadata for historic backfills.
//!
//! Owns the multi-batch outer loop and the within-batch metadata pagination
//! (one batch = up to [`DEFAULT_HISTORIC_BATCH_SIZE`] messages, fetched across
//! one or two API pages with anchor dedup). Per-batch work — body fetch,
//! decrypt, indexing, observer-side checkpoint persistence — is delegated to
//! [`HistoricMailboxWalkerObserver`].
//!
//! # Loop shape
//!
//! ```text
//! loop {
//!     fetch_metadata_batch  (within-batch pagination, network retry)
//!     observer.on_batch     (body fetch + decrypt + index + checkpoint)
//!     handle outcome:
//!         Continue          - advance anchor, fetch next batch
//!                             (or stop with Completed if batch was short)
//!         StopCompleted     - terminate with Completed
//!         StopIncomplete    - terminate with Incomplete (e.g. skipped messages)
//! }
//! ```
//!
//! Retryable network failures (metadata fetch) wait for connectivity inside
//! the walker. Retryable observer API errors do the same via
//! [`HistoricMailboxObserverError::RetryableApi`]. Non-retryable errors honour a
//! configurable retry budget with cool-down before being surfaced as
//! [`HistoricMailboxWalkOutcome::Fatal`].

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use mail_api::services::proton::ProtonMail;
use mail_api::services::proton::common::MessageId;
use mail_api::services::proton::prelude::MessageMetadata as ApiMessageMetadata;
use mail_api::services::proton::requests::GetMessagesOptions;
use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::LabelId;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

pub use crate::historic_mailbox_observer_error_code::HistoricMailboxObserverErrorCode;
pub use crate::historic_mailbox_walker_incomplete_reason::HistoricMailboxWalkerIncompleteReason;

use crate::MailUserContext;
use crate::datatypes::labels::{ScrollOrderDir, ScrollOrderField};
use crate::datatypes::{ReadFilter, SystemLabelId};

/// Default batch size: one Proton API metadata page worth of messages.
pub const DEFAULT_HISTORIC_BATCH_SIZE: usize = mail_api::MAX_PAGE_ELEMENT_COUNT;

/// Cool-down applied between non-retryable observer failures before the next
/// attempt. Mirrors the proton-bridge default.
pub const DEFAULT_HISTORIC_RETRY_COOL_DOWN: Duration = Duration::from_secs(20);

/// Maximum consecutive non-retryable observer failures before the walker
/// surfaces [`HistoricMailboxWalkOutcome::Fatal`].
pub const DEFAULT_HISTORIC_MAX_RETRY_ATTEMPTS: u32 = 3;

// -- Continuation ------------------------------------------------------------

/// Resume token for paginated historic metadata walks.
///
/// `anchor_time` + `anchor_message_id` together identify the boundary between
/// the messages already processed and the next batch (older messages, since
/// historic walks traverse newest-to-oldest).
#[derive(Debug, Clone)]
pub struct HistoricFetchContinuation {
    pub anchor_time: u64,
    pub anchor_message_id: MessageId,
}

// -- Observer trait + types --------------------------------------------------

/// Per-batch metadata yielded to the observer.
#[derive(Debug)]
pub struct HistoricMailboxBatch {
    /// Up to [`HistoricMailboxWalkerConfig::batch_size`] messages, newest
    /// first within the batch.
    pub messages: Vec<ApiMessageMetadata>,
    /// All Mail total reported by the API on the first page of this batch
    /// (`None` if not captured yet).
    pub mailbox_messages_total: Option<u64>,
    /// True when this is the tail of the mailbox (fewer messages than the
    /// configured batch size). Observers use this to decide between
    /// [`HistoricBatchOutcome::Continue`] (which the walker resolves to
    /// `Completed`) and [`HistoricBatchOutcome::StopIncomplete`].
    pub is_last_batch: bool,
}

/// Outcome of one [`HistoricMailboxWalkerObserver::on_batch`] call.
#[derive(Debug)]
pub enum HistoricBatchOutcome {
    /// Continue walking. On the last batch the walker terminates with
    /// [`HistoricMailboxWalkOutcome::Completed`]; otherwise it advances the
    /// anchor and fetches the next batch.
    Continue,
    /// Stop the walk, declaring full completion regardless of batch size.
    StopCompleted,
    /// Stop the walk and surface as
    /// [`HistoricMailboxWalkOutcome::Incomplete`] (e.g. last-batch messages
    /// were skipped and re-runs should retry the tail of the mailbox).
    StopIncomplete {
        reason: HistoricMailboxWalkerIncompleteReason,
    },
}

/// Error reported by [`HistoricMailboxWalkerObserver::on_batch`].
#[derive(Debug, thiserror::Error)]
pub enum HistoricMailboxObserverError {
    /// Transient API failure (typically network). The walker waits for
    /// connectivity then retries the same batch.
    #[error("retryable observer API error: {0}")]
    RetryableApi(#[source] ApiServiceError),
    /// Non-retryable API failure. Walker applies its retry budget with
    /// cool-down before declaring [`HistoricMailboxWalkOutcome::Fatal`].
    #[error("fatal observer API error: {0}")]
    FatalApi(#[source] ApiServiceError),
    /// Non-API failure (storage, index prepare, etc.). Walker applies its
    /// retry budget with cool-down before declaring [`HistoricMailboxWalkOutcome::Fatal`].
    #[error("fatal observer error: {0}")]
    Fatal(HistoricMailboxObserverErrorCode),
    /// Observer received cancellation. Walker terminates as
    /// [`HistoricMailboxWalkOutcome::Cancelled`].
    #[error("cancelled")]
    Cancelled,
}

impl HistoricMailboxObserverError {
    #[must_use]
    pub fn is_retryable_api(&self) -> bool {
        matches!(self, Self::RetryableApi(api) if api.is_network_failure())
    }

    /// Stable code for durable storage and downstream orchestrator mapping.
    #[must_use]
    pub fn observer_error_code(&self) -> HistoricMailboxObserverErrorCode {
        match self {
            Self::RetryableApi(_) => HistoricMailboxObserverErrorCode::RetryableNetwork,
            Self::FatalApi(_) => HistoricMailboxObserverErrorCode::FatalApi,
            Self::Fatal(code) => *code,
            Self::Cancelled => HistoricMailboxObserverErrorCode::Internal,
        }
    }

    /// Stable code persisted in `content_search_indexing_state.last_error`.
    #[cfg(feature = "foundation_search")]
    #[must_use]
    pub fn last_error_code(&self) -> mail_search::ContentSearchIndexingLastErrorCode {
        self.observer_error_code().into()
    }
}

#[async_trait]
pub trait HistoricMailboxWalkerObserver: Send + Sync {
    /// Process one batch of metadata.
    ///
    /// Implementations typically fetch + decrypt + index bodies, persist
    /// their own checkpoint reflecting per-batch progress, and return
    /// [`HistoricBatchOutcome::Continue`] for non-final batches.
    async fn on_batch(
        &self,
        ctx: &Arc<MailUserContext>,
        batch: HistoricMailboxBatch,
        cancel: CancellationToken,
    ) -> Result<HistoricBatchOutcome, HistoricMailboxObserverError>;
}

// -- Walker types ------------------------------------------------------------

/// Knobs controlling the walker's loop behaviour.
#[derive(Debug, Clone)]
pub struct HistoricMailboxWalkerConfig {
    /// Target messages per batch yielded to the observer.
    pub batch_size: usize,
    /// Cool-down between non-retryable observer failures.
    pub retry_cool_down: Duration,
    /// Maximum consecutive non-retryable observer failures before giving up.
    pub max_retry_attempts: u32,
}

impl Default for HistoricMailboxWalkerConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_HISTORIC_BATCH_SIZE,
            retry_cool_down: DEFAULT_HISTORIC_RETRY_COOL_DOWN,
            max_retry_attempts: DEFAULT_HISTORIC_MAX_RETRY_ATTEMPTS,
        }
    }
}

/// Terminal outcome of [`HistoricMailboxWalker::run`].
#[derive(Debug)]
pub enum HistoricMailboxWalkOutcome {
    /// Reached natural mailbox end (last batch with `Continue` outcome) or
    /// the observer signalled [`HistoricBatchOutcome::StopCompleted`].
    Completed,
    /// Cancellation was observed.
    Cancelled,
    /// Observer signalled [`HistoricBatchOutcome::StopIncomplete`].
    Incomplete {
        reason: HistoricMailboxWalkerIncompleteReason,
    },
    /// Retry budget exhausted or unrecoverable walker-side failure.
    Fatal { error: HistoricMailboxWalkerError },
}

/// Errors surfaced by the walker itself (not by the observer).
#[derive(Debug, thiserror::Error)]
pub enum HistoricMailboxWalkerError {
    #[error("API error: {0}")]
    Api(#[source] ApiServiceError),
    #[error("invalid continuation")]
    InvalidContinuation,
    #[error("observer fatal: {0}")]
    ObserverFatal(HistoricMailboxObserverError),
}

impl HistoricMailboxWalkerError {
    #[must_use]
    pub fn is_retryable_api(&self) -> bool {
        matches!(self, Self::Api(api) if api.is_network_failure())
    }
}

// -- Walker -----------------------------------------------------------------

/// Page-by-page historic mailbox walker.
///
/// Stateless across `run` invocations: each call drives one full walk over
/// All Mail starting from the provided continuation.
pub struct HistoricMailboxWalker {
    config: HistoricMailboxWalkerConfig,
}

impl HistoricMailboxWalker {
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(HistoricMailboxWalkerConfig::default())
    }

    #[must_use]
    pub fn with_config(config: HistoricMailboxWalkerConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn config(&self) -> &HistoricMailboxWalkerConfig {
        &self.config
    }

    /// Drive the multi-batch walk until completion, cancel, observer stop, or
    /// fatal failure.
    pub async fn run(
        &self,
        ctx: &Arc<MailUserContext>,
        observer: &dyn HistoricMailboxWalkerObserver,
        from: Option<HistoricFetchContinuation>,
        cancel: CancellationToken,
    ) -> HistoricMailboxWalkOutcome {
        if let Some(c) = &from {
            info!(
                "HistoricMailboxWalker: resuming from anchor time={} id={}",
                c.anchor_time, c.anchor_message_id
            );
        }

        let mut current_anchor: Option<HistoricFetchContinuation> = from;
        let mut retry_attempts: u32 = 0;

        loop {
            if cancel.is_cancelled() {
                return HistoricMailboxWalkOutcome::Cancelled;
            }

            let fetched = match fetch_metadata_batch_with_resilience(
                ctx,
                self.config.batch_size,
                current_anchor.as_ref(),
                &cancel,
            )
            .await
            {
                Ok(b) => b,
                Err(WalkerFetchControl::Cancelled) => {
                    return HistoricMailboxWalkOutcome::Cancelled;
                }
                Err(WalkerFetchControl::Fatal(err)) => {
                    return HistoricMailboxWalkOutcome::Fatal { error: err };
                }
            };

            // Empty response = natural mailbox end with no observer-visible
            // batch. Treat as Completed: nothing more to walk.
            if fetched.messages.is_empty() {
                info!(
                    "HistoricMailboxWalker: API returned empty page; treating as natural mailbox end"
                );
                return HistoricMailboxWalkOutcome::Completed;
            }

            let next_anchor = fetched.next_anchor();
            let is_last_batch = fetched.messages.len() < self.config.batch_size;

            let batch = HistoricMailboxBatch {
                messages: fetched.messages,
                mailbox_messages_total: fetched.mailbox_messages_total,
                is_last_batch,
            };

            match observer.on_batch(ctx, batch, cancel.clone()).await {
                Ok(HistoricBatchOutcome::Continue) => {
                    retry_attempts = 0;
                    if is_last_batch {
                        info!(
                            "HistoricMailboxWalker: natural mailbox end (last batch + observer continue)"
                        );
                        return HistoricMailboxWalkOutcome::Completed;
                    }
                    current_anchor = next_anchor;
                }
                Ok(HistoricBatchOutcome::StopCompleted) => {
                    return HistoricMailboxWalkOutcome::Completed;
                }
                Ok(HistoricBatchOutcome::StopIncomplete { reason }) => {
                    return HistoricMailboxWalkOutcome::Incomplete { reason };
                }
                Err(HistoricMailboxObserverError::Cancelled) => {
                    return HistoricMailboxWalkOutcome::Cancelled;
                }
                Err(observer_err @ HistoricMailboxObserverError::RetryableApi(_)) => {
                    warn!(
                        "HistoricMailboxWalker: retryable observer error ({observer_err}); waiting for connectivity"
                    );
                    tokio::select! {
                        () = cancel.cancelled() => {
                            return HistoricMailboxWalkOutcome::Cancelled;
                        }
                        () = wait_until_online(ctx) => {}
                    }
                    // Retry the same batch (re-fetch metadata + re-process)
                    // without advancing the anchor — matches the pre-walker
                    // resilience wrapper behaviour.
                }
                Err(observer_err @ HistoricMailboxObserverError::FatalApi(_))
                | Err(observer_err @ HistoricMailboxObserverError::Fatal(_)) => {
                    retry_attempts = retry_attempts.saturating_add(1);
                    if retry_attempts > self.config.max_retry_attempts {
                        warn!(
                            "HistoricMailboxWalker: giving up after {} failed attempts: {observer_err}",
                            retry_attempts
                        );
                        return HistoricMailboxWalkOutcome::Fatal {
                            error: HistoricMailboxWalkerError::ObserverFatal(observer_err),
                        };
                    }
                    warn!(
                        "HistoricMailboxWalker: observer attempt {}/{} failed ({observer_err}); cooling down for {:?}",
                        retry_attempts,
                        self.config.max_retry_attempts.saturating_add(1),
                        self.config.retry_cool_down,
                    );
                    tokio::select! {
                        () = cancel.cancelled() => {
                            return HistoricMailboxWalkOutcome::Cancelled;
                        }
                        () = tokio::time::sleep(self.config.retry_cool_down) => {}
                    }
                    // Retry the same batch without advancing the anchor.
                }
            }
        }
    }
}

impl Default for HistoricMailboxWalker {
    fn default() -> Self {
        Self::new()
    }
}

// -- Within-batch metadata pagination ---------------------------------------

/// One batch worth of metadata pulled across one or two API pages.
struct FetchedBatch {
    messages: Vec<ApiMessageMetadata>,
    mailbox_messages_total: Option<u64>,
}

impl FetchedBatch {
    fn next_anchor(&self) -> Option<HistoricFetchContinuation> {
        self.messages.last().map(|m| HistoricFetchContinuation {
            anchor_time: m.time,
            anchor_message_id: m.id.clone(),
        })
    }
}

/// Walker-side control flow returned by metadata fetch.
enum WalkerFetchControl {
    Cancelled,
    Fatal(HistoricMailboxWalkerError),
}

/// Fetch one batch worth of metadata with wait-for-connectivity on retryable
/// network failures. Loops internally; only returns once a non-retryable
/// outcome (success, fatal, cancel) is reached.
async fn fetch_metadata_batch_with_resilience(
    ctx: &Arc<MailUserContext>,
    batch_size: usize,
    from: Option<&HistoricFetchContinuation>,
    cancel: &CancellationToken,
) -> Result<FetchedBatch, WalkerFetchControl> {
    loop {
        if cancel.is_cancelled() {
            return Err(WalkerFetchControl::Cancelled);
        }

        match fetch_metadata_batch(ctx, batch_size, from, cancel).await {
            Ok(batch) => return Ok(batch),
            Err(WalkerFetchError::Cancelled) => return Err(WalkerFetchControl::Cancelled),
            Err(WalkerFetchError::InvalidContinuation) => {
                return Err(WalkerFetchControl::Fatal(
                    HistoricMailboxWalkerError::InvalidContinuation,
                ));
            }
            Err(WalkerFetchError::Api(api)) if api.is_network_failure() => {
                warn!(
                    "HistoricMailboxWalker: retryable network error during metadata fetch ({api}); waiting for connectivity"
                );
                tokio::select! {
                    () = cancel.cancelled() => return Err(WalkerFetchControl::Cancelled),
                    () = wait_until_online(ctx) => {}
                }
            }
            Err(WalkerFetchError::Api(api)) => {
                return Err(WalkerFetchControl::Fatal(HistoricMailboxWalkerError::Api(
                    api,
                )));
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum WalkerFetchError {
    #[error("API error: {0}")]
    Api(ApiServiceError),
    #[error("invalid continuation")]
    InvalidContinuation,
    #[error("cancelled")]
    Cancelled,
}

/// One within-batch metadata pass: 1 or 2 API pages with anchor dedup,
/// truncated to `batch_size`.
async fn fetch_metadata_batch(
    ctx: &Arc<MailUserContext>,
    batch_size: usize,
    from: Option<&HistoricFetchContinuation>,
    cancel: &CancellationToken,
) -> Result<FetchedBatch, WalkerFetchError> {
    let session = ctx.session();
    let remote_label_id = LabelId::all_mail();

    let mut total_pages: usize = usize::from(from.is_some());
    let mut last_message_id: Option<MessageId> = from.map(|c| c.anchor_message_id.clone());
    let mut last_message_time: Option<u64> = from.map(|c| c.anchor_time);
    let mut total_fetched = 0_usize;
    let mut mailbox_messages_total: Option<u64> = None;
    let mut accumulated: Vec<ApiMessageMetadata> = Vec::with_capacity(batch_size);

    loop {
        if cancel.is_cancelled() {
            return Err(WalkerFetchError::Cancelled);
        }
        if total_fetched >= batch_size {
            break;
        }

        let page_size = if total_pages == 0 {
            batch_size as u64
        } else {
            (batch_size as u64) + 1
        };

        let mut opts = GetMessagesOptions {
            label_id: Some(vec![remote_label_id.clone()]),
            page_size,
            unread: ReadFilter::All.into(),
            desc: ScrollOrderDir::Desc.as_api_desc(),
            sort: ScrollOrderField::Time.as_api_sort(),
            ..Default::default()
        };

        if total_pages > 0 {
            let Some(anchor_time) = last_message_time else {
                warn!("HistoricMailboxWalker: pagination anchor time missing after first page");
                return Err(WalkerFetchError::InvalidContinuation);
            };
            let Some(anchor_id) = last_message_id.as_ref() else {
                warn!(
                    "HistoricMailboxWalker: pagination anchor message id missing after first page"
                );
                return Err(WalkerFetchError::InvalidContinuation);
            };
            opts.anchor = Some(anchor_time);
            opts.anchor_id = Some(anchor_id.clone());
        }

        let response = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(WalkerFetchError::Cancelled),
            result = ProtonMail::get_messages(session, opts) => {
                result.map_err(WalkerFetchError::Api)?
            }
        };

        if mailbox_messages_total.is_none() && response.total > 0 {
            mailbox_messages_total = Some(response.total);
        }

        if response.messages.is_empty() {
            break;
        }

        let mut messages = response.messages;
        if total_pages > 0
            && !messages.is_empty()
            && let Some(last_id) = &last_message_id
        {
            if messages[0].id == *last_id {
                messages.remove(0);
            } else if messages.len() > batch_size {
                messages.pop();
            }
        }

        if messages.is_empty() {
            break;
        }

        let remaining = batch_size.saturating_sub(total_fetched);
        if messages.len() > remaining {
            messages.truncate(remaining);
        }

        let page_fetched = messages.len();
        total_fetched += page_fetched;

        if let Some(anchor) = messages.last() {
            last_message_id = Some(anchor.id.clone());
            last_message_time = Some(anchor.time);
        }

        accumulated.extend(messages);
        total_pages = total_pages.saturating_add(1);

        if page_fetched < batch_size {
            break;
        }
    }

    Ok(FetchedBatch {
        messages: accumulated,
        mailbox_messages_total,
    })
}

async fn wait_until_online(ctx: &Arc<MailUserContext>) {
    let mut observer = ctx.network_monitor_service().network_status_observer();
    if observer.is_online() {
        return;
    }
    observer.wait_until_online().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_walker_config_matches_expected_constants() {
        let cfg = HistoricMailboxWalkerConfig::default();
        assert_eq!(cfg.batch_size, DEFAULT_HISTORIC_BATCH_SIZE);
        assert_eq!(cfg.retry_cool_down, DEFAULT_HISTORIC_RETRY_COOL_DOWN);
        assert_eq!(cfg.max_retry_attempts, DEFAULT_HISTORIC_MAX_RETRY_ATTEMPTS);
    }

    #[test]
    fn walker_error_classifies_retryable_api() {
        let net = ApiServiceError::NetworkError("offline".into());
        let walker_err = HistoricMailboxWalkerError::Api(net);
        assert!(walker_err.is_retryable_api());
    }

    #[test]
    fn observer_error_retains_api_source_and_stable_code() {
        let api = ApiServiceError::NetworkError("offline".into());
        let err = HistoricMailboxObserverError::RetryableApi(api);
        assert!(err.is_retryable_api());
        assert_eq!(
            err.observer_error_code(),
            HistoricMailboxObserverErrorCode::RetryableNetwork
        );
    }
}
