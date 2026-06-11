use crate::AppError;
use crate::models::Message;
use crate::user_context::events::event_model::MessageEvent;
use crate::user_context::events::event_subscriber::PostEventSyncData;
use mail_action_queue::rebase::RebaseChangeSet;
use mail_core_api::services::proton::LabelId;
use mail_search::MailSearchService;
use mail_stash::stash::WriteTx;
use std::collections::HashSet;

pub async fn handle_message_events(
    tx: &WriteTx<'_>,
    events: &[MessageEvent],
    rebase_change_set: &mut RebaseChangeSet,
    data: &mut PostEventSyncData,
    unresolved_label_ids: &HashSet<LabelId>,
    search_service: Option<&MailSearchService>,
) -> Result<(), AppError> {
    for event in events {
        if let Some(id) = Message::handle_event(
            tx,
            &event.remote_id,
            event.action,
            event.message.as_ref(),
            rebase_change_set,
            unresolved_label_ids,
            search_service,
        )
        .await?
        {
            data.msg_for_prefetch.push(id);
        }
    }

    Ok(())
}
