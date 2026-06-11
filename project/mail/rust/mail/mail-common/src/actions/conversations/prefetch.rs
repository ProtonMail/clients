use crate::actions::{MailActionError, PREFETCH_ROLLBACK_ACTION_GROUP};
use crate::datatypes::{ConversationViewOptions, DeletedItemType, LocalConversationId};
use crate::models::{Conversation, DeletedItem, Message};
use crate::{MailContextError, MailUserContext};
use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionGroup, ActionId, DefaultVersionConverter, Handler,
    Priority, Type,
};

use mail_action_queue::rebase::RebaseChangeSet;
use mail_core_common::datatypes::LocalLabelId;
use mail_core_common::models::{Label, ModelIdExtension};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::WriteTx;
use serde::{self, Deserialize, Serialize};
use std::sync::Weak;
use tracing::error;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Prefetch {
    local_id: LocalConversationId,
    local_label_id: LocalLabelId,
}

impl Prefetch {
    pub fn new(local_id: LocalConversationId, local_label_id: LocalLabelId) -> Self {
        Self {
            local_id,
            local_label_id,
        }
    }
}

impl Action<UserDb> for Prefetch {
    const TYPE: Type = Type("prefetch_conversation");
    const VERSION: u32 = 1;
    const PRIORITY: Priority = Priority::Lowest;

    const GROUP: ActionGroup = PREFETCH_ROLLBACK_ACTION_GROUP;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = PrefetchHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailActionError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        ActionDependencyKeys::default()
    }
}

pub struct PrefetchHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for PrefetchHandler {
    type Action = Prefetch;

    async fn apply_local(
        &self,
        _: ActionId,
        _: &mut Self::Action,
        _: &WriteTx<'_>,
    ) -> Result<
        <Self::Action as Action<UserDb>>::LocalOutput,
        <Self::Action as Action<UserDb>>::Error,
    > {
        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        _: &mut Self::Action,
        _: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        Ok(())
    }

    async fn apply_remote(
        &self,
        _: ActionId,
        action: &mut Self::Action,
    ) -> Result<
        <Self::Action as Action<UserDb>>::RemoteOutput,
        <Self::Action as Action<UserDb>>::Error,
    > {
        tracing::trace!("Prefetching {:?}", action.local_id);

        let ctx = self.ctx.upgrade().ok_or(MailActionError::LostContext)?;
        let mut tether = ctx.user_stash().connection();
        let deleted = Conversation::is_deleted(action.local_id, &tether).await?;

        if deleted {
            tracing::debug!(
                "Conversation is deleted, skipping prefetch action, conversation_id: `{}`",
                action.local_id
            );
            return Ok(());
        }

        // Check if conversation is in deleted_items tombstone table
        if let Some(remote_id) =
            Conversation::local_id_counterpart(action.local_id, &tether).await?
        {
            let deleted_tombstones = DeletedItem::find_deleted_by_remote_ids(
                std::iter::once(remote_id.as_str()),
                DeletedItemType::Conversation,
                &tether,
            )
            .await?;

            if !deleted_tombstones.is_empty() {
                tracing::debug!(
                    "Conversation is in deleted_items, skipping prefetch action, conversation_id: `{}`",
                    action.local_id
                );
                return Ok(());
            }
        }

        let _ = Conversation::sync_conversation_messages(
            ctx.network_monitor_service(),
            action.local_id,
            &mut tether,
            ctx.session(),
            false,
            ctx.action_queue(),
            ctx.search_service(),
        )
        .await;

        let messages =
            Message::in_conversation(action.local_id, ConversationViewOptions::All, &tether)
                .await?;

        let Some(label) = Label::load(action.local_label_id, &tether).await? else {
            error!(
                "Label not found for prefetch action, label_id: `{}`",
                action.local_label_id
            );
            return Ok(());
        };

        let Some(message_id) = Conversation::focused_message(&label, &messages) else {
            error!(
                "Message id to open was not found for prefetch action, conversation_id: `{}`",
                action.local_id
            );
            return Ok(());
        };

        tracing::trace!(
            "Prefetching message {message_id} body for conversation `{local_id}`",
            local_id = action.local_id
        );

        let Some(local_message) = Message::load(message_id, &tether).await? else {
            error!(
                "Message not found for prefetch action, conversation_id: `{}`",
                action.local_id
            );
            return Ok(());
        };

        if let Err(e) = local_message.fetch_message_body(&ctx, &mut tether).await {
            match e {
                MailContextError::Api(network_error) => {
                    return Err(MailActionError::Http(network_error));
                }
                _ => {
                    error!("Error prefetching message body, details: `{e}`");
                }
            }
        }

        Ok(())
    }
    async fn rebase_local(
        &self,
        _: ActionId,
        _: &mut Self::Action,
        _: &RebaseChangeSet,
        _: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        Ok(())
    }
}
