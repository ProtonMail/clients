//! Per-page checkpoint policy for ephemeral historic load (All Mail only).

use mail_api::services::proton::common::MessageId;
use mail_search::{SearchServiceError, save_checkpoint_in_write_tx};
use mail_stash::stash::WriteTx;

/// Whether and how to update the All Mail checkpoint in the same transaction as index blobs.
#[derive(Debug, Clone)]
pub enum EphemeralPageCheckpointWrite {
    Save {
        anchor_time: u64,
        anchor_message_id: MessageId,
    },
    /// Leave the existing checkpoint row unchanged (e.g. page indexed nothing).
    Unchanged,
}

impl EphemeralPageCheckpointWrite {
    /// Advance the checkpoint only when at least one message was indexed on the page.
    #[must_use]
    pub fn from_indexed_page(oldest_indexed: Option<(u64, MessageId)>) -> Self {
        match oldest_indexed {
            Some((anchor_time, anchor_message_id)) => Self::Save {
                anchor_time,
                anchor_message_id,
            },
            None => Self::Unchanged,
        }
    }

    #[must_use]
    pub fn save(anchor_time: u64, anchor_message_id: MessageId) -> Self {
        Self::Save {
            anchor_time,
            anchor_message_id,
        }
    }

    /// Persist this checkpoint decision inside an open Stash write transaction.
    pub async fn persist_in_write_tx(&self, bond: &WriteTx<'_>) -> Result<(), SearchServiceError> {
        match self {
            Self::Save {
                anchor_time,
                anchor_message_id,
            } => save_checkpoint_in_write_tx(bond, *anchor_time, anchor_message_id).await,
            Self::Unchanged => Ok(()),
        }
    }
}

/// Cumulative batch counters to persist on the final ACID page transaction of
/// each historic-load batch (same `write_tx` as checkpoint + blobs).
#[derive(Debug, Clone, Copy)]
pub struct IndexingBatchProgressWrite {
    pub fetched: u64,
    pub indexed: u64,
    pub skipped: u64,
    pub mailbox_messages_total: Option<u64>,
}

impl IndexingBatchProgressWrite {
    #[must_use]
    pub fn from_batch_totals(
        messages_fetched: usize,
        messages_indexed: usize,
        messages_skipped_missing_body: usize,
        mailbox_messages_total: Option<u64>,
    ) -> Self {
        Self {
            fetched: messages_fetched as u64,
            indexed: messages_indexed as u64,
            skipped: messages_skipped_missing_body as u64,
            mailbox_messages_total,
        }
    }

    pub async fn persist_in_write_tx(&self, bond: &WriteTx<'_>) -> Result<(), SearchServiceError> {
        mail_search::apply_indexing_batch_progress_in_write_tx(
            bond,
            self.fetched,
            self.indexed,
            self.skipped,
            self.mailbox_messages_total,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_indexed_page_leaves_checkpoint_unchanged() {
        assert!(matches!(
            EphemeralPageCheckpointWrite::from_indexed_page(None),
            EphemeralPageCheckpointWrite::Unchanged
        ));
    }

    #[test]
    fn indexed_page_saves_oldest_anchor() {
        let id = MessageId::from("msg-oldest");
        let cp = EphemeralPageCheckpointWrite::from_indexed_page(Some((1_700_000_000, id.clone())));
        match cp {
            EphemeralPageCheckpointWrite::Save {
                anchor_time,
                anchor_message_id,
            } => {
                assert_eq!(anchor_time, 1_700_000_000);
                assert_eq!(anchor_message_id, id);
            }
            EphemeralPageCheckpointWrite::Unchanged => {
                panic!("expected Save when oldest indexed is present");
            }
        }
    }
}
