//! Resume token for paginated historic / ephemeral metadata fetches (next page = older messages).

use mail_api::services::proton::common::MessageId;
use mail_common::MailContextError;
use mail_search::{EphemeralHistoricCheckpoint, MailSearchService, SearchServiceError};

/// Next metadata page starts after this message (anchor time + id), returning the next **older**
/// page(s) in descending-time order.
#[derive(Debug, Clone)]
pub struct HistoricFetchContinuation {
    pub anchor_time: u64,
    pub anchor_message_id: MessageId,
}

impl HistoricFetchContinuation {
    /// `anchor_time` is intentionally not validated (including `0`): it is whatever the API returned
    pub fn validate(&self) -> Result<(), MailContextError> {
        if self.anchor_message_id.as_str().is_empty() {
            Err(MailContextError::Other(anyhow::anyhow!(
                "Historic fetch continuation: anchor_message_id must not be empty"
            )))
        } else {
            Ok(())
        }
    }
}

impl From<EphemeralHistoricCheckpoint> for HistoricFetchContinuation {
    fn from(cp: EphemeralHistoricCheckpoint) -> Self {
        Self {
            anchor_time: cp.anchor_time,
            anchor_message_id: cp.anchor_message_id,
        }
    }
}

/// Resolves the metadata pagination anchor for one ephemeral historic load run.
///
/// - Explicit `continuation` wins over `resume_from_checkpoint`.
/// - When `resume_from_checkpoint` is true and the DB has no row, returns `None` (start from newest).
/// - Checkpoints persisted after a run reflect the oldest **indexed** message in that batch.
pub async fn resolve_effective_continuation(
    search_service: &MailSearchService,
    continuation: Option<HistoricFetchContinuation>,
    resume_from_checkpoint: bool,
) -> Result<Option<HistoricFetchContinuation>, MailContextError> {
    if continuation.is_some() && resume_from_checkpoint {
        tracing::debug!("historic load: explicit continuation overrides resume_from_checkpoint");
    }

    let effective = match continuation {
        Some(c) => Some(c),
        None if resume_from_checkpoint => {
            let loaded = search_service
                .load_ephemeral_historic_checkpoint()
                .await
                .map_err(checkpoint_storage_err)?;
            if loaded.is_none() {
                tracing::info!(
                    "resume_from_checkpoint: no saved All Mail checkpoint; starting from newest"
                );
            }
            loaded.map(HistoricFetchContinuation::from)
        }
        None => None,
    };

    if let Some(ref c) = effective {
        c.validate()?;
    }

    Ok(effective)
}

fn checkpoint_storage_err(e: SearchServiceError) -> MailContextError {
    MailContextError::Other(anyhow::anyhow!(
        "Ephemeral historic checkpoint: failed to load: {e}"
    ))
}
