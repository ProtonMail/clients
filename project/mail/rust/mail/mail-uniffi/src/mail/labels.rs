use std::sync::Arc;

use mail_common::ProtonMailError;
use mail_uniffi_runtime::uniffi_async;

use crate::core::datatypes::Id;
use crate::errors::{ActionError, VoidActionResult};
use crate::mail::MailUserSession;
use crate::mail::datatypes::WellKnownLabelColor;

#[uniffi_export]
#[returns(VoidActionResult)]
pub async fn create_custom_folder(
    session: Arc<MailUserSession>,
    parent_id: Option<Id>,
    name: String,
    color: WellKnownLabelColor,
    notify: bool,
) -> Result<(), ActionError> {
    let user_context = session.ctx()?;
    uniffi_async(async move {
        let action = mail_common::actions::labels::Create::new_custom_folder(
            parent_id.map(Into::into),
            name,
            color.into(),
            notify,
        );
        user_context
            .action_queue()
            .queue_action(action)
            .await
            .map_err(ProtonMailError::from)?;
        Ok::<_, ProtonMailError>(())
    })
    .await
    .map_err(ActionError::from)
    .into()
}

#[uniffi_export]
#[returns(VoidActionResult)]
pub async fn create_custom_label(
    session: Arc<MailUserSession>,
    name: String,
    color: WellKnownLabelColor,
) -> Result<(), ActionError> {
    let user_context = session.ctx()?;
    uniffi_async(async move {
        let action = mail_common::actions::labels::Create::new_custom_label(name, color.into());
        user_context
            .action_queue()
            .queue_action(action)
            .await
            .map_err(ProtonMailError::from)?;
        Ok::<_, ProtonMailError>(())
    })
    .await
    .map_err(ActionError::from)
    .into()
}

/// Edit a custom folder
#[uniffi_export]
#[returns(VoidActionResult)]
pub async fn update_custom_folder(
    session: Arc<MailUserSession>,
    id: Id,
    parent_id: Option<Id>,
    name: String,
    color: WellKnownLabelColor,
    notify: bool,
) -> Result<(), ActionError> {
    let user_context = session.ctx()?;
    uniffi_async(async move {
        let action = mail_common::actions::labels::Update::new_custom_folder(
            id.into(),
            parent_id.map(Into::into),
            name,
            color.into(),
            notify,
        );
        user_context
            .action_queue()
            .queue_action(action)
            .await
            .map_err(ProtonMailError::from)?;
        Ok::<_, ProtonMailError>(())
    })
    .await
    .map_err(ActionError::from)
    .into()
}

/// Edit a custom label
#[uniffi_export]
#[returns(VoidActionResult)]
pub async fn update_custom_label(
    session: Arc<MailUserSession>,
    id: Id,
    name: String,
    color: WellKnownLabelColor,
) -> Result<(), ActionError> {
    let user_context = session.ctx()?;
    uniffi_async(async move {
        let action =
            mail_common::actions::labels::Update::new_custom_label(id.into(), name, color.into());
        user_context
            .action_queue()
            .queue_action(action)
            .await
            .map_err(ProtonMailError::from)?;
        Ok::<_, ProtonMailError>(())
    })
    .await
    .map_err(ActionError::from)
    .into()
}

/// Delete a custom label or custom folder
#[uniffi_export]
#[returns(VoidActionResult)]
pub async fn delete_label(session: Arc<MailUserSession>, label_id: Id) -> Result<(), ActionError> {
    let user_context = session.ctx()?;
    uniffi_async(async move {
        let action = mail_common::actions::labels::Delete::new(label_id.into());
        user_context
            .action_queue()
            .queue_action(action)
            .await
            .map_err(ProtonMailError::from)?;
        Ok::<_, ProtonMailError>(())
    })
    .await
    .map_err(ActionError::from)
    .into()
}
