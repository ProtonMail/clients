//! Historic search load orchestration.
//!
mod historic_load;
mod synthetic_local_message_id;

#[cfg(feature = "foundation_search_lab_harness")]
pub mod ephemeral;
#[cfg(feature = "foundation_search_lab_harness")]
pub mod ephemeral_timing;

pub use historic_load::{
    FetchAllMessagesSummary, HistoricFetchContinuation, HistoricLoadResult, fetch_all_messages,
    historic_load_messages, queue_indexing_and_prefetch, wait_until_prefetch_and_search_index_idle,
};
pub use synthetic_local_message_id::{SYNTHETIC_LOCAL_MESSAGE_ID_MIN, SyntheticLocalMessageIdSeq};

#[cfg(feature = "foundation_search_lab_harness")]
pub use ephemeral::{EphemeralHistoricLoadResult, ephemeral_index_only_messages};
#[cfg(feature = "foundation_search_lab_harness")]
pub use ephemeral_timing::EphemeralTimingStats;
