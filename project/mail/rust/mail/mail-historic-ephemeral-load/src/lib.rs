//! Ephemeral historic load: fetch metadata + bodies from the Proton API → index into Foundation Search.
//!
//! No SQLite writes — no message metadata saves, no body saves, no index intents, no queue actions.
//! Offline JSONL / remote fixture body substitution is intentionally not supported here.
//!
//! [`HistoricFetchContinuation`] is defined here for use by this orchestration and by UniFFI callers;
//! a future SQLite-backed historic load crate can depend on this crate for the shared type only.

mod continuation;
pub mod ephemeral_timing;

mod ephemeral;

pub use continuation::HistoricFetchContinuation;
pub use ephemeral::{EphemeralHistoricLoadResult, ephemeral_index_only_messages};
pub use ephemeral_timing::{EphemeralTimingCollector, EphemeralTimingStats};
