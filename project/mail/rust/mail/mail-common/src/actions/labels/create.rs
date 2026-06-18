use std::sync::Weak;

use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionId, DefaultVersionConverter, Handler, Type,
};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api_labels::LabelId;
use mail_core_common::actions::dependency_builder::{
    ActionDependencyKeysBuilder, LocalIdActionDepExt,
};
use mail_core_common::datatypes::{LabelColor, LabelType, LocalLabelId, WellKnownLabelColor};
use mail_core_common::models::{Label, LabelError, ModelExtension};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::WriteTx;
use serde::{Deserialize, Serialize};

use crate::MailUserContext;
use crate::actions::MailActionError;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Create {
    local_id: Option<LocalLabelId>,
    local_parent_id: Option<LocalLabelId>,
    label_type: LabelType,
    name: String,
    color: LabelColor,
    notify: bool,
}

impl Create {
    pub fn new_custom_folder(
        parent_local_id: Option<LocalLabelId>,
        name: String,
        color: WellKnownLabelColor,
        notify: bool,
    ) -> Self {
        Self::new(parent_local_id, LabelType::Folder, name, color, notify)
    }

    pub fn new_custom_label(name: String, color: WellKnownLabelColor) -> Self {
        Self::new(None, LabelType::Label, name, color, true)
    }

    pub fn new(
        parent_local_id: Option<LocalLabelId>,
        label_type: LabelType,
        name: String,
        color: WellKnownLabelColor,
        notify: bool,
    ) -> Self {
        Self {
            local_id: None,
            local_parent_id: parent_local_id,
            color: color.into(),
            label_type,
            name,
            notify,
        }
    }
}

impl Action<UserDb> for Create {
    const TYPE: Type = Type("create_label");
    const VERSION: u32 = 1;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = CreateHandler;
    type RemoteOutput = LabelId;
    type LocalOutput = ();
    type Error = MailActionError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        ActionDependencyKeysBuilder::new()
            .with_optional_related_many(self.local_parent_id)
            .record(self.local_id.unwrap().to_create_dependency_key())
            .build()
    }
}

pub struct CreateHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for CreateHandler {
    type Action = Create;

    async fn apply_local(
        &self,
        _: ActionId,
        action: &mut Create,
        tx: &WriteTx<'_>,
    ) -> Result<(), MailActionError> {
        let mut new_label = Label {
            local_id: None,
            remote_id: None,
            local_parent_id: action.local_parent_id,
            remote_parent_id: None,
            color: action.color.clone(),
            display: true,
            display_order: Label::max_descendants_display_order(tx, action.local_parent_id).await?
                + 1,
            expanded: false,
            label_type: action.label_type,
            name: action.name.clone(),
            notify: action.notify,
            path: None,
            sticky: false,
            last_unseen_message: None,
        };
        new_label.save(tx).await?;
        action.local_id = new_label.local_id;
        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        action: &mut Create,
        tx: &WriteTx<'_>,
    ) -> Result<(), MailActionError> {
        let Some(local_id) = action.local_id else {
            return Ok(());
        };
        Label::delete_by_id(local_id, tx).await?;
        Ok(())
    }

    async fn apply_remote(
        &self,
        _: ActionId,
        action: &mut Create,
    ) -> Result<LabelId, MailActionError> {
        let ctx = self.ctx.upgrade().ok_or(MailActionError::LostContext)?;
        let mut tether = ctx.user_stash().connection();
        let Some(local_id) = action.local_id else {
            return Err(MailActionError::Label(LabelError::LabelWithoutIds));
        };
        let remote_parent_id = if let Some(local_parent_id) = action.local_parent_id {
            Some(Label::resolve_remote_label_id(local_parent_id, &tether).await?)
        } else {
            None
        };
        let mut label = Label::create_remote(
            action.label_type,
            action.name.clone(),
            action.color.clone(),
            remote_parent_id.clone(),
            action.notify,
            ctx.session(),
        )
        .await?;
        label.local_id = Some(local_id);
        label.local_parent_id = action.local_parent_id;
        label.remote_parent_id = remote_parent_id;
        let remote_id = label.remote_id.clone();
        tether
            .write_tx::<_, (), <Self::Action as Action<UserDb>>::Error>(async move |tx| {
                label.save(tx).await?;
                Ok(())
            })
            .await?;
        let Some(remote_id) = remote_id else {
            return Err(MailActionError::Label(LabelError::LabelWithoutIds));
        };
        Ok(remote_id)
    }

    async fn rebase_local(
        &self,
        _id: ActionId,
        _action: &mut Create,
        _change_set: &RebaseChangeSet,
        _tx: &WriteTx<'_>,
    ) -> Result<(), MailActionError> {
        // Create does not need to be rebased because it is creating a brand new thing
        Ok(())
    }
}
