use crate::actions::PREFETCH_ROLLBACK_ACTION_GROUP;
use crate::models::RollbackItem;
use crate::{MailContextError, MailUserContext};
use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionGroup, ActionId, DefaultVersionConverter, Handler,
    Priority, Type,
};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_core_common::actions::dependency_builder::ActionDependencyKeysBuilder;
use mail_stash::UserDb;
use mail_stash::stash::WriteTx;
use serde::{Deserialize, Serialize};
use std::sync::Weak;

/// This action flushes the rollback items.
///
/// Rollback items are items that need to refetched from the server when something fails. The
/// failure often happens because we are out of sync.
#[derive(Debug, Serialize, Deserialize)]
pub struct RollbackAction {}

const ROLLBACK_BATCH_SIZE: usize = 50;

impl Action<UserDb> for RollbackAction {
    const TYPE: Type = Type("item_rollback");
    const VERSION: u32 = 1;
    const PRIORITY: Priority = Priority::Low;

    const GROUP: ActionGroup = PREFETCH_ROLLBACK_ACTION_GROUP;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = RollbackActionHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailContextError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        ActionDependencyKeysBuilder::new().build()
    }
}

pub struct RollbackActionHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for RollbackActionHandler {
    type Action = RollbackAction;

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
        _: &mut Self::Action,
    ) -> Result<
        <Self::Action as Action<UserDb>>::RemoteOutput,
        <Self::Action as Action<UserDb>>::Error,
    > {
        let ctx = self
            .ctx
            .upgrade()
            .ok_or_else(|| MailContextError::LostContext)?;
        let mut tether = ctx.user_stash().connection();
        RollbackItem::sync_all(
            ctx.session(),
            &mut tether,
            Some(ROLLBACK_BATCH_SIZE),
            ctx.action_queue(),
        )
        .await?;

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
