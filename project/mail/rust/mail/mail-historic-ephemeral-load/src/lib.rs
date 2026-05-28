//! Ephemeral historic load: per-batch body fetch + Foundation Search index for the historic
//! mailbox walker (defined in [`mail_common::historic_mailbox_walker`]).
//!
//! This crate is the content-search-specific *observer* on top of the walker: the walker owns
//! the multi-batch outer loop and within-batch metadata pagination, while this crate processes
//! each batch (body fetch + decrypt + index + checkpoint + cumulative progress) via
//! [`ContentSearchHistoricObserver`] and orchestrates the per-session lifecycle (durable status,
//! in-process one-run-at-a-time slot, cancel-and-wait) via
//! [`ContentSearchIndexingOrchestrator`].
//!
//! Persists message metadata for successfully indexed messages only (no bodies, index intents,
//! or prefetch queue), atomically with Foundation Search blobs per sub-chunk. The All Mail
//! checkpoint and cumulative batch progress are persisted in one transaction at the end of each
//! walker batch (same size as [`EPHEMERAL_BODY_SUBCHUNK_SIZE`]). Offline JSONL / remote fixture body substitution is intentionally not supported
//! here.

/// Messages per historic-load walker batch (one All Mail metadata page).
///
/// Kept equal to [`EPHEMERAL_BODY_SUBCHUNK_SIZE`] so each metadata page is one body-fetch +
/// index ACID unit (single sub-chunk, single persist transaction before checkpoint advance).
pub const EPHEMERAL_HISTORIC_LOAD_BATCH_SIZE: usize = 20;

/// Body fetch / decrypt / index persist unit within one walker batch.
///
/// Must match [`EPHEMERAL_HISTORIC_LOAD_BATCH_SIZE`] (one sub-chunk per metadata page).
pub const EPHEMERAL_BODY_SUBCHUNK_SIZE: usize = 20;

// If batch size exceeds sub-chunk size, a retryable body-fetch failure after an earlier
// sub-chunk has persisted would re-fetch bodies for messages already indexed in that page.
// Implement body-fetch caching (or equivalent) before relaxing this invariant.
const _BATCH_AND_SUBCHUNK_SIZES_MATCH: () =
    assert!(EPHEMERAL_HISTORIC_LOAD_BATCH_SIZE == EPHEMERAL_BODY_SUBCHUNK_SIZE);
const _SUBCHUNK_IS_NONZERO: () = assert!(EPHEMERAL_BODY_SUBCHUNK_SIZE > 0);

#[cfg(test)]
mod tests {
    use super::{EPHEMERAL_BODY_SUBCHUNK_SIZE, EPHEMERAL_HISTORIC_LOAD_BATCH_SIZE};

    #[test]
    fn ephemeral_batch_and_subchunk_sizes_match() {
        assert_eq!(
            EPHEMERAL_HISTORIC_LOAD_BATCH_SIZE, EPHEMERAL_BODY_SUBCHUNK_SIZE,
            "walker metadata page size and body ACID unit must stay aligned; \
             if they diverge, add body-fetch caching before persisting partial sub-chunks"
        );
    }
}

mod checkpoint;
mod continuation;
pub mod ephemeral_timing;
mod error;

mod ephemeral;
mod historic_indexing_service;
mod observer;
mod orchestrator;

#[cfg(test)]
mod integration_tests;

pub use checkpoint::{EphemeralPageCheckpointWrite, IndexingBatchProgressWrite};
pub use continuation::{HistoricFetchContinuation, resolve_effective_continuation};
pub use ephemeral::{EphemeralHistoricLoadResult, ephemeral_index_batch};
pub use ephemeral_timing::{EphemeralTimingCollector, EphemeralTimingStats};
pub use error::EphemeralHistoricLoadError;
pub use historic_indexing_service::historic_indexing_provider;
pub use mail_search::ContentSearchStartOutcome;
pub use observer::ContentSearchHistoricObserver;
pub use orchestrator::{
    ContentSearchIndexingOrchestrator, ContentSearchIndexingOrchestratorConfig,
    DEFAULT_CANCEL_WAIT_BEFORE_CLEAR, DEFAULT_CONCURRENT_BODY_FETCHES, DEFAULT_MAX_RETRY_ATTEMPTS,
    DEFAULT_RETRY_COOL_DOWN, IndexingRunOutcome, StartDecision, persist_run_outcome,
};
