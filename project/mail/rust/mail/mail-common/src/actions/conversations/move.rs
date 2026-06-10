use crate::actions::{ActionMoveData, MailActionError};
use crate::models::Conversation;
use crate::{AppError, MailUserContext};
use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionId, FactoryResult, Handler, Type, VersionConverter,
};
use mail_action_queue::enqueue;
use mail_action_queue::queue::Queue;
use mail_action_queue::rebase::RebaseChangeSet;
use mail_stash::UserDb;
use mail_stash::stash::{Tether, WriteTx};
use serde::{Deserialize, Serialize};
use std::sync::Weak;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Move(pub ActionMoveData<Conversation>);

impl VersionConverter<UserDb> for Move {
    type Output = Self;

    fn convert(old_version: u32, _: u32, data: &[u8]) -> FactoryResult<Self::Output> {
        ActionMoveData::convert(old_version, data).map(Move)
    }
}

impl Action<UserDb> for Move {
    const TYPE: Type = Type("move_conversations");
    const VERSION: u32 = 3;
    type VersionConverter = Self;
    type Handler = MoveHandler;
    type RemoteOutput = ();
    type LocalOutput = Self;
    type Error = MailActionError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        self.0.action_dependency_keys()
    }
}

pub struct MoveHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for MoveHandler {
    type Action = Move;

    async fn apply_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        tx: &WriteTx<'_>,
    ) -> Result<Self::Action, <Self::Action as Action<UserDb>>::Error> {
        let mut action_2 = action.clone();
        let action_2 = tx
            .sync_bridge(|tx| {
                action_2.0.move_to(tx)?;
                Ok(action_2)
            })
            .await?;
        *action = action_2;
        Ok(action.clone())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        action.0.revert_local(tx).await?;
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
        action.0.apply_remote(ctx.session(), &mut tether).await
    }

    async fn rebase_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        rebase_change_set: &RebaseChangeSet,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        action.0.rebase_local(rebase_change_set, tx).await?;
        Ok(())
    }
}

pub struct UndoMoveToConversations {
    pub action: Move,
    pub id: ActionId,
}

impl UndoMoveToConversations {
    pub async fn undo(self, queue: &Queue<UserDb>, _: &Tether) -> Result<(), AppError> {
        if queue.cancel(self.id).await.is_ok() {
            // The undoing is done by the revert_local of the action.
            return Ok(());
        };
        // The queue couldn't revert. This means that we're on our own to undo this.

        let (label_as, mark_unread) = self.action.0.build_undo_states();

        if let Some(mark_unread) = mark_unread {
            enqueue!(queue, [label_as, mark_unread])?;
        } else {
            enqueue!(queue, [label_as])?;
        }

        Ok(())
    }
}
