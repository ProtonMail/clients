//! Search indexing event handler
//!
//! This module handles search indexing for message events, following the subscriber pattern.
//! It provides functions that can be called from event handlers to queue search indexing
//! operations within the same transaction for atomicity.
//!
//! TODO(ET-5871): If bodies are synced but not stored, indexed data may become orphaned
//! because the local message ID used by intents is no longer referenceable.

use crate::AppError;
use mail_api::services::proton::common::MessageId;
use mail_core_common::event_loop::events::Action;
use mail_stash::stash::WriteTx;

use crate::search::MailSearchService;

/// Handle search indexing for a single message event (v6 event structure)
///
/// This function handles search indexing for a single message event, used by the v6 event subscriber.
/// It takes the message ID, action, and local message ID (if available).
///
/// # Arguments
///
/// * `tx` - Database transaction bond
/// * `remote_id` - Remote message ID
/// * `action` - Action type (Create, Update, Delete, etc.)
/// * `local_id` - Optional local message ID (if message was created/updated)
///
/// # Returns
///
/// Returns `Ok(())` on success, or an `AppError` if indexing operations fail.
pub async fn handle_search_indexing_for_message(
    tx: &WriteTx<'_>,
    remote_id: &MessageId,
    action: Action,
    local_id: Option<u64>,
    search_service: &MailSearchService,
) -> Result<(), AppError> {
    use crate::models::Message;
    use mail_core_common::models::ModelIdExtension;
    match action {
        Action::Delete => {
            // If we have a local_id, use it directly. Otherwise, look it up.
            let local_id = if let Some(id) = local_id {
                id
            } else if let Ok(Some(msg)) =
                Message::remote_id_counterpart(remote_id.clone(), tx).await
            {
                msg.as_u64()
            } else {
                return Ok(()); // Message not found, nothing to remove
            };

            // Queue search removal
            if let Err(e) = search_service.queue_remove(local_id, tx).await {
                tracing::warn!(
                    "Failed to queue search removal for message {}: {}",
                    local_id,
                    e
                );
            }
        }

        Action::Create | Action::Update | Action::UpdateFlags => {
            // If we have a local_id, use it directly. Otherwise, look it up.
            let local_id = if let Some(id) = local_id {
                id
            } else if let Ok(Some(msg)) =
                Message::remote_id_counterpart(remote_id.clone(), tx).await
            {
                msg.as_u64()
            } else {
                return Ok(()); // Message not found, nothing to index
            };

            if let Err(e) = search_service.queue_index(local_id, tx).await {
                tracing::warn!(
                    "Failed to queue search indexing for message {}: {}",
                    local_id,
                    e
                );
            }
        }
    }

    Ok(())
}
