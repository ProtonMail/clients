//! Foundation Search integration for local email search
//!
//! This module provides:
//! - `StashMessageDataProvider` - Provides message data for search indexing
//! - `search_local_with_keywords` - Searches using keywords and converts remote IDs to local IDs
//!
//! The search service, worker, and other components are now in `proton-mail-search` crate.
//! This module only contains the mail-common specific wiring.

#[cfg(feature = "foundation_search")]
pub mod data_provider;

pub mod content_search_historic_indexing;
#[cfg(feature = "foundation_search")]
pub mod search_results;

#[cfg(feature = "foundation_search")]
pub use data_provider::StashMessageDataProvider;

pub use content_search_historic_indexing::*;

#[cfg(feature = "foundation_search")]
pub use search_results::{LocalSearchResult, SearchMatchPosition, search_local_with_keywords};

#[cfg(feature = "foundation_search")]
// Re-export from proton-mail-search crate for convenience
pub use mail_search::{
    BlobStorage, CleanupResult, ContentSearchIndexingLastErrorCode, ContentSearchIndexingProgress,
    ContentSearchIndexingState, ContentSearchIndexingStatus, FoundEntry, FoundationSearchEngine,
    IndexResult, IndexStats, LocalMessageId, MailSearchService, MessageDataProvider,
    RateLimitedWatcherHandle, SearchError, SearchIndexIntent, SearchIndexWorker, SearchOperation,
    SearchServiceError, SearchStats, StashBlobStorage, WorkerShutdownHandle, WorkerShutdownSignal,
};
