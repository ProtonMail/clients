use crate::actions::MailActionError;
use crate::{AppError, MailUserContext};
use anyhow::anyhow;
use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionId, DefaultVersionConverter, Handler, Type,
};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api::services::proton::ProtonMail;
use mail_core_api::services::proton::{EventId, LabelId};
use mail_core_common::actions::dependency_builder::{
    ActionDependencyKeysBuilder, LocalIdActionDepExt,
};
use mail_core_common::datatypes::{LocalLabelId, SystemLabel};
use mail_core_common::event_loop::event_store::{MAIL_EVENT_TYPE_ID, load_event_id};
use mail_core_common::models::{Label, LabelError};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::WriteTx;
use serde::{Deserialize, Serialize};
use std::sync::Weak;
use tracing::{debug, error};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarkSeen {
    local_id: LocalLabelId,
    remote_id: Option<LabelId>,
    unseen_msg_marker: Option<EventId>,
}

impl MarkSeen {
    pub fn new(local_id: LocalLabelId) -> Self {
        Self {
            local_id,
            remote_id: None,
            unseen_msg_marker: None,
        }
    }
}

impl Action<UserDb> for MarkSeen {
    const TYPE: Type = Type("mark_label_seen");
    const VERSION: u32 = 1;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = MarkSeenHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailActionError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        ActionDependencyKeysBuilder::new()
            .with_required_related(self.local_id)
            .with_required(
                self.local_id
                    .to_custom_dependency_key("mail-mark-label-seen"),
            )
            .build()
    }
}

pub struct MarkSeenHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for MarkSeenHandler {
    type Action = MarkSeen;

    async fn apply_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        let mut label = Label::load(action.local_id, tx)
            .await?
            .ok_or_else(|| AppError::LabelNotFound(action.local_id))?;

        if !SystemLabel::from_opt_rid(label.remote_id.as_ref()).is_some_and(|sl| sl.is_category()) {
            error!("Non category label cannot be marked as seen");
            return Err(MailActionError::Label(LabelError::ExpectedCategoryLabel));
        }

        if label.last_unseen_message.is_none() {
            debug!(
                "Category label {:?} has no unseen message; nothing to mark as seen",
                action.local_id
            );
            return Ok(());
        }

        action.remote_id = label.remote_id.clone();
        action.unseen_msg_marker = label.last_unseen_message.clone();

        debug!(
            "Marking category label {:?} as seen. LocalId: `{:?}`, Marker `{:?}`",
            action.remote_id, action.local_id, action.unseen_msg_marker
        );

        label.last_unseen_message = None;

        label.save(tx).await?;

        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        let mut label = Label::load(action.local_id, tx)
            .await?
            .ok_or_else(|| AppError::LabelNotFound(action.local_id))?;

        label.last_unseen_message = action.unseen_msg_marker.clone();

        label.save(tx).await?;

        Ok(())
    }

    async fn apply_remote(
        &self,
        _: ActionId,
        action: &mut Self::Action,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        let Some(remote_id) = action.remote_id.clone() else {
            debug!("Label had no unseen message; skipping remote mark-seen call");
            return Ok(());
        };

        let ctx = self.ctx.upgrade().ok_or(MailActionError::LostContext)?;
        let Some(event_id) = load_event_id(ctx.user_context(), MAIL_EVENT_TYPE_ID).await? else {
            return Err(MailActionError::Other(anyhow!(
                "Missing latest mail event id to mark label as seen. Reverting..."
            )));
        };

        debug!(
            "Label {:?} seen request - latest mail event id: {:?}",
            remote_id, event_id
        );

        ctx.session()
            .post_label_seen(remote_id, event_id.into())
            .await?;

        Ok(())
    }

    async fn rebase_local(
        &self,
        this_id: ActionId,
        action: &mut Self::Action,
        _: &RebaseChangeSet,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        self.apply_local(this_id, action, tx).await?;
        Ok(())
    }
}
