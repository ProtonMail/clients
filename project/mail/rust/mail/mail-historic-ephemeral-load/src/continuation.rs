//! Resume token for paginated historic / ephemeral metadata fetches (next page = older messages).

use mail_api::services::proton::common::MessageId;

/// Next metadata page starts after this message (anchor time + id), returning the next **older**
/// page(s) in descending-time order.
#[derive(Debug, Clone)]
pub struct HistoricFetchContinuation {
    pub anchor_time: u64,
    pub anchor_message_id: MessageId,
}
