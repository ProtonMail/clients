use std::sync::Weak;

use crate::MailUserContext;
use crate::actions::MailActionError;
use crate::models::IncomingDefault;
use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionId, DefaultVersionConverter, Handler, Type,
};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_core_common::actions::dependency_builder::ActionDependencyKeysBuilder;
use mail_stash::UserDb;
use mail_stash::stash::WriteTx;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SyncIncomingDefaults;

impl Action<UserDb> for SyncIncomingDefaults {
    const TYPE: Type = Type("update_incoming_defaults");
    const VERSION: u32 = 1;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = SyncIncomingDefaultsHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailActionError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        ActionDependencyKeysBuilder::new().build()
    }
}

pub struct SyncIncomingDefaultsHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for SyncIncomingDefaultsHandler {
    type Action = SyncIncomingDefaults;

    async fn apply_local(
        &self,
        _: ActionId,
        _: &mut Self::Action,
        _: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
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
        let ctx = self.ctx.upgrade().ok_or(MailActionError::LostContext)?;
        let data = IncomingDefault::sync(ctx.session()).await?;

        tracing::info!("Updating incoming defaults");

        let mut tether = ctx.user_stash().connection();
        tether
            .write_tx::<_, _, <Self::Action as Action<UserDb>>::Error>(async |tx| {
                IncomingDefault::replace_all(
                    data.into_iter().map(IncomingDefault::from).collect(),
                    tx,
                )
                .await?;
                Ok(())
            })
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
