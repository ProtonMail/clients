//! UniFFI surface for content search historic indexing.
//!
//! Owns the public UniFFI types (`ContentSearchIndexingStatus`,
//! `ContentSearchStartOutcome`, `ContentSearchIndexingProgress`,
//! [`WatchContentSearchIndexingStream`]) and their conversions from
//! `mail-search` types exposed via `mail-common::search`.
//!
//! Session-bound async exports live on [`MailUserSession`] in
//! `user_session.rs`.

use std::sync::{Arc, Weak};

use mail_common::search::{
    ContentSearchIndexingLastErrorCode as RealLastErrorCode,
    ContentSearchIndexingProgress as RealProgress, ContentSearchIndexingStatus as RealStatus,
    ContentSearchStartOutcome as RealStartOutcome, RateLimitedWatcherHandle,
};
use mail_common::{
    MailContextError, MailUserContext, ProtonMailError as RealProtonMailError, Unexpected,
};
use mail_uniffi_runtime::async_runtime;
use tokio_util::sync::CancellationToken;

use crate::errors::unexpected::UnexpectedError;
use crate::errors::{ProtonError, UserSessionError};

// -- Status enum -----------------------------------------------------------

/// Lifecycle of the historic indexing orchestrator, as seen by mobile.
///
/// Mirrors [`mail_search::ContentSearchIndexingStatus`]. A distinct
/// `Failed` variant is intentionally not modelled — non-clean exits land
/// in [`Self::Interrupted`] with [`ContentSearchIndexingLastErrorCode`]
/// populated (queried via the watch stream or
/// `content_search_get_indexing_progress`).
#[derive(Debug, Clone, Copy, Eq, PartialEq, uniffi::Enum)]
pub enum ContentSearchIndexingStatus {
    None,
    Ongoing,
    Interrupted,
    Completed,
}

impl From<RealStatus> for ContentSearchIndexingStatus {
    fn from(value: RealStatus) -> Self {
        match value {
            RealStatus::None => Self::None,
            RealStatus::Ongoing => Self::Ongoing,
            RealStatus::Interrupted => Self::Interrupted,
            RealStatus::Completed => Self::Completed,
        }
    }
}

// -- Start outcome enum ----------------------------------------------------

/// Outcome of `content_search_start_indexing`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, uniffi::Enum)]
pub enum ContentSearchStartOutcome {
    NoWork,
    Started,
    AlreadyRunning,
}

impl From<RealStartOutcome> for ContentSearchStartOutcome {
    fn from(value: RealStartOutcome) -> Self {
        match value {
            RealStartOutcome::NoWork => Self::NoWork,
            RealStartOutcome::Started => Self::Started,
            RealStartOutcome::AlreadyRunning => Self::AlreadyRunning,
        }
    }
}

/// Stable failure reason for an interrupted historic indexing run.
///
/// Mirrors [`mail_search::ContentSearchIndexingLastErrorCode`]. Mobile maps
/// each variant to a localized string.
#[derive(Debug, Clone, Copy, Eq, PartialEq, uniffi::Enum)]
pub enum ContentSearchIndexingLastErrorCode {
    RetryableNetwork,
    FatalApi,
    CheckpointStorage,
    IndexingStateStorage,
    IndexPrepare,
    PagePersist,
    MetadataPrepare,
    InvalidContinuation,
    IncompleteWithSkippedBodies,
    StaleOngoingRecovered,
    Internal,
}

impl From<RealLastErrorCode> for ContentSearchIndexingLastErrorCode {
    fn from(value: RealLastErrorCode) -> Self {
        match value {
            RealLastErrorCode::RetryableNetwork => Self::RetryableNetwork,
            RealLastErrorCode::FatalApi => Self::FatalApi,
            RealLastErrorCode::CheckpointStorage => Self::CheckpointStorage,
            RealLastErrorCode::IndexingStateStorage => Self::IndexingStateStorage,
            RealLastErrorCode::IndexPrepare => Self::IndexPrepare,
            RealLastErrorCode::PagePersist => Self::PagePersist,
            RealLastErrorCode::MetadataPrepare => Self::MetadataPrepare,
            RealLastErrorCode::InvalidContinuation => Self::InvalidContinuation,
            RealLastErrorCode::IncompleteWithSkippedBodies => Self::IncompleteWithSkippedBodies,
            RealLastErrorCode::StaleOngoingRecovered => Self::StaleOngoingRecovered,
            RealLastErrorCode::Internal => Self::Internal,
        }
    }
}

// -- Progress record ------------------------------------------------------

/// Snapshot of indexing progress for settings UI and foreground notifications.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ContentSearchIndexingProgress {
    pub status: ContentSearchIndexingStatus,
    pub enabled: bool,
    pub messages_indexed_total: u64,
    pub messages_fetched_total: u64,
    pub messages_skipped_total: u64,
    pub batches_completed: u64,
    pub last_error: Option<ContentSearchIndexingLastErrorCode>,
    pub estimated_fraction: Option<f64>,
}

impl From<RealProgress> for ContentSearchIndexingProgress {
    fn from(value: RealProgress) -> Self {
        Self {
            status: value.status.into(),
            enabled: value.enabled,
            messages_indexed_total: value.messages_indexed_total,
            messages_fetched_total: value.messages_fetched_total,
            messages_skipped_total: value.messages_skipped_total,
            batches_completed: value.batches_completed,
            last_error: value
                .last_error
                .map(ContentSearchIndexingLastErrorCode::from),
            estimated_fraction: value.estimated_fraction,
        }
    }
}

// -- Watch stream ----------------------------------------------------------

/// Rate-limited stream of [`ContentSearchIndexingProgress`] snapshots.
///
/// Same family as [`crate::mail::user_session::WatchUserStream`]: subscribe
/// once, read [`Self::initial_progress`], then call [`Self::next_async`] for
/// each subsequent update (each tick loads a fresh snapshot from SQLite).
#[derive(uniffi::Object)]
pub struct WatchContentSearchIndexingStream {
    initial_progress: ContentSearchIndexingProgress,
    handle: RateLimitedWatcherHandle,
    token: CancellationToken,
    ctx: Weak<MailUserContext>,
}

impl WatchContentSearchIndexingStream {
    pub(crate) async fn new(ctx: Arc<MailUserContext>) -> Result<Arc<Self>, RealProtonMailError> {
        let search_service = ctx.search_service();
        let initial = search_service
            .load_indexing_progress()
            .await
            .map(ContentSearchIndexingProgress::from)
            .map_err(map_proton_internal("content_search_watch_indexing_stream"))?;
        let handle = search_service
            .watch_indexing_state()
            .await
            .map_err(map_proton_internal("content_search_watch_indexing_stream"))?;

        Ok(Arc::new(Self {
            initial_progress: initial,
            handle,
            token: ctx.create_child_cancellation_token(),
            ctx: ctx.as_weak(),
        }))
    }
}

#[uniffi_export]
impl WatchContentSearchIndexingStream {
    #[must_use]
    pub fn initial_progress(&self) -> ContentSearchIndexingProgress {
        self.initial_progress.clone()
    }

    #[tracing::instrument(name = "ContentSearchIndexingStream::next", skip_all)]
    pub async fn next_async(
        self: Arc<Self>,
    ) -> Result<ContentSearchIndexingProgress, UserSessionError> {
        async_runtime()
            .spawn(async move {
                let future = self.handle.receiver().recv_async();
                self.token
                    .run_until_cancelled(future)
                    .await
                    .ok_or_else(|| {
                        map_internal("content_search_watch_indexing_stream_next")(
                            MailContextError::TaskCancelled,
                        )
                    })?
                    .map_err(|_| {
                        map_internal("content_search_watch_indexing_stream_next")("watcher closed")
                    })?;

                let ctx = self.ctx.upgrade().ok_or_else(|| {
                    map_internal("content_search_watch_indexing_stream_next")(
                        MailContextError::MissingContext,
                    )
                })?;

                ctx.search_service()
                    .load_indexing_progress()
                    .await
                    .map(ContentSearchIndexingProgress::from)
                    .map_err(map_internal("content_search_watch_indexing_stream_next"))
            })
            .await
            .map_err(|e| map_internal("content_search_watch_indexing_stream_next")(e))?
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }
}

/// Map errors to [`RealProtonMailError`] for use inside [`uniffi_async`].
pub(crate) fn map_proton_internal<E: std::fmt::Display>(
    scope: &'static str,
) -> impl FnOnce(E) -> RealProtonMailError {
    move |e| {
        tracing::error!("{scope}: {e}");
        RealProtonMailError::Unexpected(Unexpected::Internal)
    }
}

/// Map any error producible by the content-search APIs to the generic
/// `Internal` UniFFI error after logging the underlying cause.
pub(crate) fn map_internal<E: std::fmt::Display>(
    scope: &'static str,
) -> impl FnOnce(E) -> UserSessionError {
    move |e| {
        tracing::error!("{scope}: {e}");
        UserSessionError::Other(ProtonError::Unexpected(UnexpectedError::Internal))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_conversion_covers_every_real_variant() {
        for (real, expected) in [
            (RealStatus::None, ContentSearchIndexingStatus::None),
            (RealStatus::Ongoing, ContentSearchIndexingStatus::Ongoing),
            (
                RealStatus::Interrupted,
                ContentSearchIndexingStatus::Interrupted,
            ),
            (
                RealStatus::Completed,
                ContentSearchIndexingStatus::Completed,
            ),
        ] {
            let mapped: ContentSearchIndexingStatus = real.into();
            assert_eq!(mapped, expected, "status mismatch for {real:?}");
        }
    }

    #[test]
    fn start_outcome_conversion_preserves_idempotent_branches() {
        assert_eq!(
            ContentSearchStartOutcome::from(RealStartOutcome::NoWork),
            ContentSearchStartOutcome::NoWork
        );
        assert_eq!(
            ContentSearchStartOutcome::from(RealStartOutcome::Started),
            ContentSearchStartOutcome::Started
        );
        assert_eq!(
            ContentSearchStartOutcome::from(RealStartOutcome::AlreadyRunning),
            ContentSearchStartOutcome::AlreadyRunning
        );
    }
}
