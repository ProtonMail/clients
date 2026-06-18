use std::sync::Weak;

use mail_action_queue::action::{Action, ActionId, DefaultVersionConverter, Handler, Type};
use mail_action_queue::rebase::{RebaseChangeSet, RebaseKey};
use mail_core_common::actions::dependency_builder::ActionDependencyKeysBuilder;
use mail_core_common::datatypes::{LabelColor, LabelType, LocalLabelId, WellKnownLabelColor};
use mail_core_common::models::{Label, LabelError};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::WriteTx;
use serde::{Deserialize, Serialize};

use crate::MailUserContext;
use crate::actions::MailActionError;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct UpdateValues {
    local_parent_id: Option<LocalLabelId>,
    name: String,
    color: LabelColor,
    notify: bool,
    display_order: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Update {
    local_id: LocalLabelId,
    new_values: UpdateValues,
    old_values: Option<UpdateValues>,
}

impl Update {
    pub fn new_custom_folder(
        local_id: LocalLabelId,
        parent_id: Option<LocalLabelId>,
        name: String,
        color: WellKnownLabelColor,
        notify: bool,
    ) -> Self {
        Self::new(
            local_id,
            UpdateValues {
                local_parent_id: parent_id,
                name,
                color: color.into(),
                notify,
                display_order: 0,
            },
        )
    }

    pub fn new_custom_label(
        local_id: LocalLabelId,
        name: String,
        color: WellKnownLabelColor,
    ) -> Self {
        Self::new(
            local_id,
            UpdateValues {
                local_parent_id: None,
                name,
                color: color.into(),
                notify: false,
                display_order: 0,
            },
        )
    }

    fn new(local_id: LocalLabelId, new_values: UpdateValues) -> Self {
        Self {
            local_id,
            new_values,
            old_values: None,
        }
    }
}

impl Action<UserDb> for Update {
    const TYPE: Type = Type("update_label");
    const VERSION: u32 = 1;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = UpdateHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailActionError;

    fn dependency_keys(&self) -> mail_action_queue::action::ActionDependencyKeys {
        ActionDependencyKeysBuilder::new()
            .with_required_related(self.local_id)
            .with_optional_related_many(self.new_values.local_parent_id)
            .with_optional_related_many(self.old_values.as_ref().and_then(|v| v.local_parent_id))
            .build()
    }
}

pub struct UpdateHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for UpdateHandler {
    type Action = Update;

    async fn apply_local(
        &self,
        _: ActionId,
        action: &mut Update,
        tx: &WriteTx<'_>,
    ) -> Result<(), MailActionError> {
        let Some(mut label) = Label::load(action.local_id, tx).await? else {
            return Err(MailActionError::Label(LabelError::LabelWithoutIds));
        };
        action.old_values = Some(UpdateValues {
            local_parent_id: label.local_parent_id,
            name: label.name,
            color: label.color,
            notify: label.notify,
            display_order: label.display_order,
        });
        if label.local_parent_id != action.new_values.local_parent_id {
            debug_assert_eq!(label.label_type, LabelType::Folder);
            // Parent changed update display order
            label.display_order =
                Label::max_descendants_display_order(tx, action.new_values.local_parent_id).await?
                    + 1;
        }
        label.local_parent_id = action.new_values.local_parent_id;
        label.name = action.new_values.name.clone();
        label.color = action.new_values.color.clone();
        label.notify = action.new_values.notify;
        label.save(tx).await?;
        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        action: &mut Update,
        tx: &WriteTx<'_>,
    ) -> Result<(), MailActionError> {
        let Some(mut label) = Label::load(action.local_id, tx).await? else {
            return Err(MailActionError::Label(LabelError::LabelWithoutIds));
        };
        let Some(old_values) = action.old_values.take() else {
            return Err(MailActionError::Other(anyhow::anyhow!("no old values")));
        };
        label.local_parent_id = old_values.local_parent_id;
        label.display_order = old_values.display_order;
        label.name = old_values.name.clone();
        label.color = old_values.color.clone();
        label.notify = old_values.notify;
        label.save(tx).await?;
        Ok(())
    }

    async fn apply_remote(&self, _: ActionId, action: &mut Update) -> Result<(), MailActionError> {
        let ctx = self.ctx.upgrade().ok_or(MailActionError::LostContext)?;
        let mut tether = ctx.user_stash().connection();
        let remote_id = Label::resolve_remote_label_id(action.local_id, &tether).await?;
        let parent_remote_id = match action.new_values.local_parent_id {
            Some(id) => Some(Label::resolve_remote_label_id(id, &tether).await?),
            None => None,
        };

        let mut label = Label::put_remote(
            remote_id,
            action.new_values.name.clone(),
            parent_remote_id,
            action.new_values.color.clone(),
            Some(action.new_values.notify),
            ctx.session(),
        )
        .await?;
        label.local_id = Some(action.local_id);
        label.local_parent_id = action.new_values.local_parent_id;
        tether
            .write_tx::<_, _, <Self::Action as Action<UserDb>>::Error>(async move |tx| {
                label.save(tx).await?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    async fn rebase_local(
        &self,
        id: ActionId,
        action: &mut Update,
        change_set: &RebaseChangeSet,
        tx: &WriteTx<'_>,
    ) -> Result<(), MailActionError> {
        let rebase_key: RebaseKey = action.local_id.into();
        if change_set.contains(&rebase_key) {
            self.apply_local(id, action, tx).await?;
        }
        Ok(())
    }
}
