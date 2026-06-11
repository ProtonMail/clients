//! Stable error codes surfaced by [`HistoricMailboxWalkerObserver`](crate::historic_mailbox_walker::HistoricMailboxWalkerObserver)
//! implementations.
//!
//! These are content-search-agnostic identifiers for retry/fatal observer
//! failures. Downstream orchestrators map them to their own durable codes
//! (e.g. [`mail_search::ContentSearchIndexingLastErrorCode`]) via matching
//! `as_db_str()` values.

/// Stable observer failure code carried through the historic mailbox walker.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum HistoricMailboxObserverErrorCode {
    RetryableNetwork,
    FatalApi,
    CheckpointStorage,
    IndexingStateStorage,
    IndexPrepare,
    PagePersist,
    MetadataPrepare,
    InvalidContinuation,
    Internal,
}

impl HistoricMailboxObserverErrorCode {
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::RetryableNetwork => "retryable_network",
            Self::FatalApi => "fatal_api",
            Self::CheckpointStorage => "checkpoint_storage",
            Self::IndexingStateStorage => "indexing_state_storage",
            Self::IndexPrepare => "index_prepare",
            Self::PagePersist => "page_persist",
            Self::MetadataPrepare => "metadata_prepare",
            Self::InvalidContinuation => "invalid_continuation",
            Self::Internal => "internal",
        }
    }

    #[must_use]
    pub fn from_db_str(s: &str) -> Option<Self> {
        Some(match s {
            "retryable_network" => Self::RetryableNetwork,
            "fatal_api" => Self::FatalApi,
            "checkpoint_storage" => Self::CheckpointStorage,
            "indexing_state_storage" => Self::IndexingStateStorage,
            "index_prepare" => Self::IndexPrepare,
            "page_persist" => Self::PagePersist,
            "metadata_prepare" => Self::MetadataPrepare,
            "invalid_continuation" => Self::InvalidContinuation,
            "internal" => Self::Internal,
            _ => return None,
        })
    }
}

impl std::fmt::Display for HistoricMailboxObserverErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

impl From<HistoricMailboxObserverErrorCode> for mail_search::ContentSearchIndexingLastErrorCode {
    fn from(code: HistoricMailboxObserverErrorCode) -> Self {
        match code {
            HistoricMailboxObserverErrorCode::RetryableNetwork => Self::RetryableNetwork,
            HistoricMailboxObserverErrorCode::FatalApi => Self::FatalApi,
            HistoricMailboxObserverErrorCode::CheckpointStorage => Self::CheckpointStorage,
            HistoricMailboxObserverErrorCode::IndexingStateStorage => Self::IndexingStateStorage,
            HistoricMailboxObserverErrorCode::IndexPrepare => Self::IndexPrepare,
            HistoricMailboxObserverErrorCode::PagePersist => Self::PagePersist,
            HistoricMailboxObserverErrorCode::MetadataPrepare => Self::MetadataPrepare,
            HistoricMailboxObserverErrorCode::InvalidContinuation => Self::InvalidContinuation,
            HistoricMailboxObserverErrorCode::Internal => Self::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_str_round_trips_for_every_variant() {
        for code in [
            HistoricMailboxObserverErrorCode::RetryableNetwork,
            HistoricMailboxObserverErrorCode::FatalApi,
            HistoricMailboxObserverErrorCode::CheckpointStorage,
            HistoricMailboxObserverErrorCode::IndexingStateStorage,
            HistoricMailboxObserverErrorCode::IndexPrepare,
            HistoricMailboxObserverErrorCode::PagePersist,
            HistoricMailboxObserverErrorCode::MetadataPrepare,
            HistoricMailboxObserverErrorCode::InvalidContinuation,
            HistoricMailboxObserverErrorCode::Internal,
        ] {
            let s = code.as_db_str();
            assert_eq!(
                HistoricMailboxObserverErrorCode::from_db_str(s),
                Some(code),
                "round-trip failed for {s}"
            );
        }
    }
}
