//! Content-search observer for the historic mailbox walker.
//!
//! The walker (in [`mail_common::historic_mailbox_walker`]) owns the
//! multi-batch outer loop and within-batch metadata pagination. This module
//! plugs the content-search-specific per-batch work into that loop:
//!
//! - Body fetch + decrypt + HTML→text (`ephemeral_index_batch`)
//! - Foundation Search index prepare + persist (per sub-chunk)
//! - All Mail checkpoint advance + cumulative batch progress (per batch)
//! - Walker outcome mapping (continue / stop completed / stop incomplete)
//!
//! Retryable body-fetch failures inside [`ephemeral_index_batch`] surface as
//! [`HistoricMailboxObserverError::RetryableApi`] so the walker re-runs the
//! batch after waiting for connectivity. Other batch failures surface as
//! [`HistoricMailboxObserverError::FatalApi`] / [`HistoricMailboxObserverError::Fatal`]
//! and feed into the walker's retry
//! budget / cool-down policy.

use std::sync::Arc;

use async_trait::async_trait;
use mail_common::MailUserContext;
use mail_common::historic_mailbox_walker::{
    HistoricBatchOutcome, HistoricMailboxBatch, HistoricMailboxObserverError,
    HistoricMailboxWalkerIncompleteReason, HistoricMailboxWalkerObserver,
};
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::ephemeral::ephemeral_index_batch;
use crate::error::EphemeralHistoricLoadError;

/// Observer that drives content-search indexing for every batch yielded by
/// the historic mailbox walker.
pub struct ContentSearchHistoricObserver {
    concurrent_body_fetches: usize,
}

impl ContentSearchHistoricObserver {
    #[must_use]
    pub fn new(concurrent_body_fetches: usize) -> Self {
        Self {
            concurrent_body_fetches,
        }
    }
}

#[async_trait]
impl HistoricMailboxWalkerObserver for ContentSearchHistoricObserver {
    async fn on_batch(
        &self,
        ctx: &Arc<MailUserContext>,
        batch: HistoricMailboxBatch,
        cancel: CancellationToken,
    ) -> Result<HistoricBatchOutcome, HistoricMailboxObserverError> {
        let HistoricMailboxBatch {
            messages,
            mailbox_messages_total,
            is_last_batch,
        } = batch;

        let result = ephemeral_index_batch(
            ctx,
            self.concurrent_body_fetches,
            messages,
            mailbox_messages_total,
            cancel,
        )
        .await;

        let result = match result {
            Ok(r) => r,
            Err(EphemeralHistoricLoadError::Cancelled) => {
                return Err(HistoricMailboxObserverError::Cancelled);
            }
            Err(EphemeralHistoricLoadError::RetryableApi(api)) => {
                return Err(HistoricMailboxObserverError::RetryableApi(api));
            }
            Err(EphemeralHistoricLoadError::FatalApi(api)) => {
                return Err(HistoricMailboxObserverError::FatalApi(api));
            }
            Err(err) => {
                return Err(HistoricMailboxObserverError::Fatal(
                    err.observer_error_code(),
                ));
            }
        };

        // A would-be completion batch with skipped bodies must not declare
        // Completed: the skipped messages are real gaps in coverage at the
        // tail of the mailbox. Stop here as Incomplete so the next start
        // re-runs the loop and gets another chance to fetch the bodies.
        if is_last_batch && result.messages_skipped_missing_body > 0 {
            let messages_skipped = result.messages_skipped_missing_body as u64;
            warn!(
                "ContentSearchHistoricObserver: partial final batch (fetched={} skipped={}); surfacing as Incomplete instead of Completed",
                result.messages_fetched, messages_skipped,
            );
            return Ok(HistoricBatchOutcome::StopIncomplete {
                reason: HistoricMailboxWalkerIncompleteReason::SkippedBodiesAtTail,
            });
        }

        Ok(HistoricBatchOutcome::Continue)
    }
}
