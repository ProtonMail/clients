//! Classified errors for ephemeral historic load (retryable network vs fatal API/storage).

use mail_common::MailContextError;
use mail_common::historic_mailbox_walker::{
    HistoricMailboxObserverErrorCode, HistoricMailboxWalkerIncompleteReason,
};
use mail_core_api::service::ApiServiceError;
use mail_search::{ContentSearchIndexingLastErrorCode, SearchServiceError};
use mail_stash::stash::StashError;

/// Error from one ephemeral historic-load invocation.
#[derive(Debug, thiserror::Error)]
pub enum EphemeralHistoricLoadError {
    /// Transient network failure; safe to retry the same batch later.
    #[error("Retryable network error: {0}")]
    RetryableApi(#[source] ApiServiceError),

    /// Non-retryable API failure (auth, server error, bad request, etc.).
    #[error("API error: {0}")]
    FatalApi(#[source] ApiServiceError),

    /// SQLite checkpoint read/write failed.
    #[error("Checkpoint storage failed: {0}")]
    CheckpointStorage(#[source] SearchServiceError),

    /// Content search indexing-state row read/write failed.
    #[error("Indexing state storage failed: {0}")]
    IndexingState(#[source] SearchServiceError),

    /// Foundation Search in-memory prepare failed (SQLite not yet touched).
    #[error("Search index prepare failed: {0}")]
    IndexPrepare(#[source] SearchServiceError),

    /// ACID page persist failed (blobs or other Stash work in one transaction).
    #[error("Page persist failed: {0}")]
    PagePersist(#[source] StashError),

    /// Label/contact dependency resolution or related mail DB work failed.
    #[error("Metadata prepare failed: {0}")]
    MetadataPrepare(MailContextError),

    /// Explicit or stored continuation anchor is invalid.
    #[error("Invalid continuation")]
    InvalidContinuation,

    /// Operation was cancelled (e.g. user context dropped).
    #[error("Cancelled")]
    Cancelled,

    /// A spawned batch is already in progress for this session.
    #[error("Ephemeral historic load already running")]
    AlreadyRunning,

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl EphemeralHistoricLoadError {
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::RetryableApi(_))
    }

    #[must_use]
    pub fn is_fatal_api(&self) -> bool {
        matches!(self, Self::FatalApi(_))
    }

    pub fn from_api(err: ApiServiceError) -> Self {
        if err.is_network_failure() {
            Self::RetryableApi(err)
        } else {
            Self::FatalApi(err)
        }
    }

    pub fn from_search_prepare(err: SearchServiceError) -> Self {
        Self::IndexPrepare(err)
    }

    pub fn from_search_checkpoint_load(err: SearchServiceError) -> Self {
        Self::CheckpointStorage(err)
    }

    pub fn from_indexing_state(err: SearchServiceError) -> Self {
        Self::IndexingState(err)
    }

    pub fn page_persist(err: StashError) -> Self {
        Self::PagePersist(err)
    }

    pub fn from_mail_context(err: MailContextError) -> Self {
        match err {
            MailContextError::Api(api) if api.is_network_failure() => Self::RetryableApi(api),
            MailContextError::Api(api) => Self::FatalApi(api),
            MailContextError::TaskCancelled => Self::Cancelled,
            other => Self::MetadataPrepare(other),
        }
    }

    /// Stable code carried through the historic mailbox walker observer boundary.
    #[must_use]
    pub fn observer_error_code(&self) -> HistoricMailboxObserverErrorCode {
        match self {
            Self::RetryableApi(_) => HistoricMailboxObserverErrorCode::RetryableNetwork,
            Self::FatalApi(_) => HistoricMailboxObserverErrorCode::FatalApi,
            Self::CheckpointStorage(_) => HistoricMailboxObserverErrorCode::CheckpointStorage,
            Self::IndexingState(_) => HistoricMailboxObserverErrorCode::IndexingStateStorage,
            Self::IndexPrepare(_) => HistoricMailboxObserverErrorCode::IndexPrepare,
            Self::PagePersist(_) => HistoricMailboxObserverErrorCode::PagePersist,
            Self::MetadataPrepare(_) => HistoricMailboxObserverErrorCode::MetadataPrepare,
            Self::InvalidContinuation => HistoricMailboxObserverErrorCode::InvalidContinuation,
            Self::Cancelled | Self::AlreadyRunning | Self::Other(_) => {
                HistoricMailboxObserverErrorCode::Internal
            }
        }
    }

    /// Stable code persisted in `content_search_indexing_state.last_error`.
    #[must_use]
    pub fn last_error_code(&self) -> ContentSearchIndexingLastErrorCode {
        self.observer_error_code().into()
    }
}

/// Map walker incomplete reasons to durable indexing-state `last_error` values.
#[must_use]
pub fn last_error_code_from_incomplete(
    reason: HistoricMailboxWalkerIncompleteReason,
) -> ContentSearchIndexingLastErrorCode {
    match reason {
        HistoricMailboxWalkerIncompleteReason::SkippedBodiesAtTail => {
            ContentSearchIndexingLastErrorCode::IncompleteWithSkippedBodies
        }
    }
}

impl From<StashError> for EphemeralHistoricLoadError {
    fn from(err: StashError) -> Self {
        Self::MetadataPrepare(MailContextError::Stash(err))
    }
}

impl From<MailContextError> for EphemeralHistoricLoadError {
    fn from(err: MailContextError) -> Self {
        Self::from_mail_context(err)
    }
}

impl From<EphemeralHistoricLoadError> for MailContextError {
    fn from(err: EphemeralHistoricLoadError) -> Self {
        match err {
            EphemeralHistoricLoadError::RetryableApi(api)
            | EphemeralHistoricLoadError::FatalApi(api) => Self::Api(api),
            EphemeralHistoricLoadError::CheckpointStorage(err)
            | EphemeralHistoricLoadError::IndexPrepare(err) => Self::Other(err.into_inner()),
            EphemeralHistoricLoadError::PagePersist(err) => Self::Stash(err),
            EphemeralHistoricLoadError::IndexingState(err) => Self::Other(err.into_inner()),
            EphemeralHistoricLoadError::MetadataPrepare(ctx) => ctx,
            EphemeralHistoricLoadError::InvalidContinuation => {
                Self::Other(anyhow::anyhow!("Invalid continuation"))
            }
            EphemeralHistoricLoadError::Cancelled => Self::TaskCancelled,
            EphemeralHistoricLoadError::AlreadyRunning => {
                Self::Other(anyhow::anyhow!("Ephemeral historic load already running"))
            }
            EphemeralHistoricLoadError::Other(e) => Self::Other(e),
        }
    }
}

impl From<EphemeralHistoricLoadError> for mail_common::ProtonMailError {
    fn from(err: EphemeralHistoricLoadError) -> Self {
        MailContextError::from(err).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_common::historic_mailbox_walker::HistoricMailboxObserverError;
    use mail_core_api::service::ApiServiceError;

    #[test]
    fn last_error_code_maps_variants_to_stable_codes() {
        let err =
            EphemeralHistoricLoadError::page_persist(StashError::Custom(anyhow::anyhow!("boom")));
        assert_eq!(
            err.last_error_code(),
            ContentSearchIndexingLastErrorCode::PagePersist
        );
    }

    #[test]
    fn observer_error_code_matches_last_error_code() {
        let err = EphemeralHistoricLoadError::from_search_checkpoint_load(
            SearchServiceError::Checkpoint("load failed".into()),
        );
        assert_eq!(
            err.observer_error_code(),
            HistoricMailboxObserverErrorCode::CheckpointStorage
        );
        assert_eq!(
            err.last_error_code(),
            ContentSearchIndexingLastErrorCode::CheckpointStorage
        );
    }

    #[test]
    fn classifies_network_as_retryable() {
        let err =
            EphemeralHistoricLoadError::from_api(ApiServiceError::NetworkError("offline".into()));
        assert!(err.is_retryable());
        assert!(!err.is_fatal_api());
    }

    #[test]
    fn retryable_api_propagates_through_observer_error_mapping() {
        let api = ApiServiceError::Timeout("slow".into());
        let observer = HistoricMailboxObserverError::RetryableApi(api);
        assert!(observer.is_retryable_api());
        assert_eq!(
            observer.observer_error_code(),
            HistoricMailboxObserverErrorCode::RetryableNetwork
        );
        assert_eq!(
            observer.last_error_code(),
            ContentSearchIndexingLastErrorCode::RetryableNetwork
        );
    }

    #[test]
    fn classifies_unauthorized_as_fatal() {
        let err = EphemeralHistoricLoadError::from_api(ApiServiceError::Unauthorized(
            "expired".into(),
            None,
        ));
        assert!(!err.is_retryable());
        assert!(err.is_fatal_api());
    }

    #[test]
    fn retryable_api_maps_to_mail_context_api() {
        let err = EphemeralHistoricLoadError::from_api(ApiServiceError::Timeout("slow".into()));
        let ctx = MailContextError::from(err);
        assert!(ctx.is_network_failure());
        assert!(matches!(ctx, MailContextError::Api(_)));
    }

    #[test]
    fn indexing_state_maps_to_mail_context_other_with_message() {
        let err = EphemeralHistoricLoadError::from_indexing_state(
            SearchServiceError::IndexingState("load failed".into()),
        );
        let ctx = MailContextError::from(err);
        assert!(matches!(ctx, MailContextError::Other(_)));
        assert!(
            ctx.to_string().contains("Indexing state storage failed"),
            "expected full SearchServiceError display, got: {ctx}"
        );
        assert!(ctx.to_string().contains("load failed"));
    }
}
