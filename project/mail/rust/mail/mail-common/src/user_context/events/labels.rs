use crate::AppError;
use crate::models::{ConversationCounter, MessageCounter};
use mail_core_api::services::proton::LabelId;
use mail_core_common::datatypes::LabelType;
use mail_core_common::event_loop::events::{Action, LabelEvent};
use mail_core_common::models::{Label, ModelIdExtension};
use mail_stash::orm::Model;
use mail_stash::stash::WriteTx;

pub async fn handle_counters_label_events(
    tx: &WriteTx<'_>,
    label_events: &[LabelEvent],
) -> Result<(), AppError> {
    for label_event in label_events {
        handle_counters_label_event(
            tx,
            &label_event.remote_id,
            label_event.action,
            label_event.label.as_ref(),
        )
        .await?;
    }
    Ok(())
}

pub async fn handle_counters_label_event(
    tx: &WriteTx<'_>,
    id: &LabelId,
    action: Action,
    label: Option<&Label>,
) -> Result<(), AppError> {
    if action == Action::Create
        && let Some(label) = label
        && label.label_type != LabelType::ContactGroup
    {
        tracing::info!("Creating message and conversation counters for {id:?}",);
        let local_id = Label::remote_id_counterpart(id.clone(), tx)
            .await?
            .ok_or_else(|| AppError::RemoteLabelDoesNotExist(id.clone()))?;
        MessageCounter::new(local_id).save(tx).await?;
        ConversationCounter::new(local_id).save(tx).await?;
    }
    Ok(())
}
