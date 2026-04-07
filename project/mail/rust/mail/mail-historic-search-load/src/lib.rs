//! Historic search load orchestration.
//!
mod historic_load;
mod synthetic_local_message_id;

pub use historic_load::{
    HistoricLoadResult, fetch_all_messages, historic_load_messages, queue_indexing_and_prefetch,
    wait_until_prefetch_and_search_index_idle,
};
pub use synthetic_local_message_id::{SYNTHETIC_LOCAL_MESSAGE_ID_MIN, SyntheticLocalMessageIdSeq};
