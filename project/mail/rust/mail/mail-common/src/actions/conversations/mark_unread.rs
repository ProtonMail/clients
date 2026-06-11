use crate::MailUserContext;
use crate::actions::{
    ConversationOrMessage, GenericActionData, GenericLabelRelatedActionData, MailActionError,
    filter_responses_by_codes,
};
use crate::datatypes::{LocalConversationId, RollbackItemType};
use crate::models::{Conversation, Message};
use anyhow::Context;
use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionId, DefaultVersionConverter, Handler, Type,
};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_core_api::consts::General;
use mail_core_common::datatypes::LocalLabelId;
use mail_core_common::models::ModelIdExtension;
use mail_stash::UserDb;
use mail_stash::exports::Transaction;
use mail_stash::stash::WriteTx;
use serde::{Deserialize, Serialize};
use std::sync::Weak;
use tracing::error;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarkUnread(GenericLabelRelatedActionData<Conversation>);

impl MarkUnread {
    pub fn new(label_id: LocalLabelId, ids: impl IntoIterator<Item = LocalConversationId>) -> Self {
        // TODO(db-tests): label_id was present in the original action, why was it used.
        Self(GenericLabelRelatedActionData::new(label_id, ids))
    }
}

impl Action<UserDb> for MarkUnread {
    const TYPE: Type = Type("mark_conversations_unread");
    const VERSION: u32 = 1;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = MarkUnreadHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailActionError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        self.0.read_unread_action_dependency_keys().build()
    }
}

pub struct MarkUnreadHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for MarkUnreadHandler {
    type Action = MarkUnread;

    async fn apply_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        let label_id = action.0.label_id;
        action
            .0
            .apply_changes_sync(tx, move |id, tx| {
                Conversation::mark_unread(label_id, [id], tx)
            })
            .await?;
        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        let modified_message_ids = action.0.modified_message_ids();
        Message::mark_read_async(modified_message_ids, tx).await?;
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
        let ctx = self.ctx.upgrade().ok_or(MailActionError::LostContext)?;
        let mut tether = ctx.user_stash().connection();

        // API call return an error 2501(Conversation was not updated) for conversation already unread
        let (remote_label_id, remote_target_ids) = action.0.resolve_ids(&tether).await?;
        if remote_target_ids.is_empty() {
            return Ok(());
        }
        let responses = Conversation::mark_multiple_as_unread_remote(
            remote_target_ids,
            remote_label_id,
            ctx.session(),
        )
        .await?;

        // In this case General::NotExists is returned also for conversations already marked as unread
        let failed_ids = filter_responses_by_codes(
            responses,
            &[General::NoError as u32, General::NotExists as u32],
        );

        if !failed_ids.is_empty() {
            error!("Mark unread operation failed for: {:?}", failed_ids);
            tether
                .sync_write_tx(move |tx: &Transaction<'_>| {
                    GenericActionData::<Conversation>::mark_rollback_sync(
                        &failed_ids,
                        RollbackItemType::Conversation,
                        tx,
                    )?;
                    let local_ids = Conversation::remote_ids_counterpart_sync(&failed_ids, tx)?;

                    Conversation::mark_read(local_ids, tx)
                        .context("Failed to rollback failed conversations")?;
                    Ok(())
                })
                .await?;
        }
        Ok(())
    }

    async fn rebase_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        changeset: &RebaseChangeSet,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        let label_id = action.0.label_id;
        action
            .0
            .rebase_changes_sync(changeset, tx, move |id, modified, tx| {
                // Reset the previously modified message back to read to reset the calculation.
                if !modified.is_empty() {
                    Message::mark_read(modified.iter().copied(), tx)?;
                }
                Conversation::mark_unread(label_id, [id], tx)
            })
            .await?;
        Ok(())
    }
}
