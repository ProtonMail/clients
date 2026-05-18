use crate::MailContextError;
use crate::models::LabelExt;
use mail_api::services::proton::prelude::RunningTasks;
use mail_core_api::services::proton::LabelId;
use mail_core_common::models::{Label, ModelIdExtension};
use mail_stash::stash::Tether;
use std::ops::ControlFlow;
use tracing::{info, instrument};

#[instrument(skip_all, fields(?remote_ids))]
pub async fn ensure_labels_are_idle(
    tether: &mut Tether,
    remote_ids: &[LabelId],
    tasks: &RunningTasks,
) -> Result<ControlFlow<()>, MailContextError> {
    let labels = Label::find_by_remote_ids(remote_ids.to_vec(), tether).await?;

    for label in labels {
        if label.is_busy(tether).await? {
            if tasks.has(label.remote_id.as_ref().unwrap()) {
                info!("Label is busy, pretending the response was empty");

                return Ok(ControlFlow::Break(()));
            } else {
                tether
                    .write_tx(async |bond| label.mark_idle(bond).await)
                    .await?;
            }
        }
    }

    Ok(ControlFlow::Continue(()))
}
