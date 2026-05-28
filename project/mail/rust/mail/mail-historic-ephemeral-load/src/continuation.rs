//! Resume token for paginated historic / ephemeral metadata fetches (next page = older messages).
//!
//! The continuation type itself lives in [`mail_common::historic_mailbox_walker`]
//! (the walker owns the loop and consumes the anchor); this module keeps the
//! content-search-specific helpers for loading the anchor from the saved
//! Foundation Search checkpoint.

pub use mail_common::historic_mailbox_walker::HistoricFetchContinuation;
use mail_search::{EphemeralHistoricCheckpoint, MailSearchService};

use crate::error::EphemeralHistoricLoadError;

/// Materialise the persistent All Mail checkpoint as a walker continuation.
#[must_use]
pub fn continuation_from_checkpoint(cp: EphemeralHistoricCheckpoint) -> HistoricFetchContinuation {
    HistoricFetchContinuation {
        anchor_time: cp.anchor_time,
        anchor_message_id: cp.anchor_message_id,
    }
}

/// Resolves the metadata pagination anchor for one ephemeral historic load run.
///
/// - Explicit `continuation` wins over `resume_from_checkpoint`.
/// - When `resume_from_checkpoint` is true and the DB has no row, returns
///   `None` (start from newest).
/// - Checkpoints persisted after a run reflect the oldest **indexed** message
///   in that batch.
pub async fn resolve_effective_continuation(
    search_service: &MailSearchService,
    continuation: Option<HistoricFetchContinuation>,
    resume_from_checkpoint: bool,
) -> Result<Option<HistoricFetchContinuation>, EphemeralHistoricLoadError> {
    if continuation.is_some() && resume_from_checkpoint {
        tracing::debug!("historic load: explicit continuation overrides resume_from_checkpoint");
    }

    let effective = match continuation {
        Some(c) => Some(c),
        None if resume_from_checkpoint => {
            let loaded = search_service
                .load_ephemeral_historic_checkpoint()
                .await
                .map_err(EphemeralHistoricLoadError::from_search_checkpoint_load)?;
            if loaded.is_none() {
                tracing::info!(
                    "resume_from_checkpoint: no saved All Mail checkpoint; starting from newest"
                );
            }
            loaded.map(continuation_from_checkpoint)
        }
        None => None,
    };

    Ok(effective)
}
