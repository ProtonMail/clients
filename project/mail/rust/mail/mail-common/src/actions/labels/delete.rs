use std::sync::Weak;

use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionId, DefaultVersionConverter, Handler, Type,
};
use mail_action_queue::rebase::{RebaseChangeSet, RebaseKey};
use mail_api_labels::{LabelApi, LabelId};
use mail_core_common::actions::dependency_builder::{
    ActionDependencyKeysBuilder, LocalIdActionDepExt as _,
};
use mail_core_common::datatypes::LocalLabelId;
use mail_core_common::models::{Label, ModelIdExtension};
use mail_stash::UserDb;
use mail_stash::stash::WriteTx;
use serde::{Deserialize, Serialize};

use crate::actions::MailActionError;
use crate::{AppError, MailUserContext};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delete {
    local_id: LocalLabelId,
    descendants_local_ids_marked_deleted: Vec<LocalLabelId>,
    remote_id: Option<LabelId>,
}

impl Delete {
    pub fn new(local_id: LocalLabelId) -> Self {
        Self {
            local_id,
            descendants_local_ids_marked_deleted: Vec::new(),
            remote_id: None,
        }
    }
}

impl Action<UserDb> for Delete {
    const TYPE: Type = Type("label_delete");
    const VERSION: u32 = 1;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = DeleteHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailActionError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        ActionDependencyKeysBuilder::new()
            .with_required(self.local_id.to_create_dependency_key())
            .build()
    }
}

pub struct DeleteHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for DeleteHandler {
    type Action = Delete;

    async fn apply_local(
        &self,
        _: ActionId,
        action: &mut Delete,
        tx: &WriteTx<'_>,
    ) -> Result<(), MailActionError> {
        let mut descendants_local_ids = Label::find_descendants(tx, action.local_id).await?;
        descendants_local_ids.push(action.local_id);
        Label::mark_deleted(tx, descendants_local_ids.clone(), true).await?;
        debug_assert!(action.descendants_local_ids_marked_deleted.is_empty());
        action.descendants_local_ids_marked_deleted = descendants_local_ids;
        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        action: &mut Delete,
        tx: &WriteTx<'_>,
    ) -> Result<(), MailActionError> {
        let ids = action.descendants_local_ids_marked_deleted.clone();
        Label::mark_deleted(tx, ids, false).await?;
        action.descendants_local_ids_marked_deleted.clear();
        Ok(())
    }

    async fn apply_remote(&self, _: ActionId, action: &mut Delete) -> Result<(), MailActionError> {
        let ctx = self.ctx.upgrade().ok_or(MailActionError::LostContext)?;
        let tether = ctx.user_stash().connection();
        let remote_id = match action.remote_id.clone() {
            Some(remote_id) => remote_id,
            None => Label::local_id_counterpart(action.local_id, &tether)
                .await?
                .ok_or_else(|| AppError::LabelDoesNotHaveRemoteId(action.local_id))?,
        };
        ctx.session().delete_label(remote_id).await?;
        Ok(())
    }

    async fn rebase_local(
        &self,
        id: ActionId,
        action: &mut Delete,
        change_set: &RebaseChangeSet,
        tx: &WriteTx<'_>,
    ) -> Result<(), MailActionError> {
        let needs_rebase = action
            .descendants_local_ids_marked_deleted
            .iter()
            .any(|&local_id| {
                let rebase_key: RebaseKey = local_id.into();
                change_set.contains(&rebase_key)
            });
        if needs_rebase {
            self.revert_local(id, action, tx).await?;
            self.apply_local(id, action, tx).await?;
        }
        Ok(())
    }
}
