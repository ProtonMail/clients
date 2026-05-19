//! Ephemeral historic load: fetch metadata + bodies from the Proton API → index into Foundation Search.
//!
//! Persists message metadata (no bodies, index intents, or prefetch queue). Offline JSONL / remote
//! fixture body substitution is intentionally not supported here.
//!
//! [`HistoricFetchContinuation`] is defined here for use by this orchestration and by UniFFI callers.

mod continuation;
pub mod ephemeral_timing;

mod ephemeral;

pub use continuation::{HistoricFetchContinuation, resolve_effective_continuation};
pub use ephemeral::{EphemeralHistoricLoadResult, ephemeral_index_only_messages};
pub use ephemeral_timing::{EphemeralTimingCollector, EphemeralTimingStats};
