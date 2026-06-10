use crate::MailUserContext;
use crate::actions::MailActionError;
use crate::actions::addresses::incoming_defaults_dependency_key;
use crate::datatypes::LocalIncomingDefaultId;
use crate::models::{IncomingDefault, IncomingDefaultLocation};
use anyhow::anyhow;
use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionId, DefaultVersionConverter, Handler, Type,
};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api::services::proton::ProtonMail;
use mail_core_api::services::proton::PrivateEmail;
use mail_core_common::actions::dependency_builder::ActionDependencyKeysBuilder;
use mail_core_common::models::ModelExtension;
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::WriteTx;
use serde::{Deserialize, Serialize};
use std::sync::Weak;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Unblock {
    pub email: PrivateEmail,
    #[serde(default)]
    removed: Option<LocalIncomingDefaultId>,
}

impl Unblock {
    pub fn new(email: PrivateEmail) -> Self {
        Self {
            email,
            removed: None,
        }
    }
}

impl Action<UserDb> for Unblock {
    const TYPE: Type = Type("unblock");
    const VERSION: u32 = 1;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = UnblockHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailActionError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        ActionDependencyKeysBuilder::new()
            .with_required(incoming_defaults_dependency_key(&self.email))
            .build()
    }
}

pub struct UnblockHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for UnblockHandler {
    type Action = Unblock;

    async fn apply_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        bond: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        tracing::info!("Unblocking {}", action.email);

        let Some(mut incoming) =
            IncomingDefault::by_email(action.email.clone().as_clear_text_str(), bond).await?
        else {
            tracing::error!(
                "Unable to unblock address that is not registered as blocked: {}",
                action.email
            );
            // Let's make this action idempotent.
            return Ok(());
        };
        if incoming.location != IncomingDefaultLocation::Blocked {
            tracing::error!(
                "Unable to unblock address that is not registered as blocked: {}",
                action.email
            );
            return Ok(());
        }
        action.removed = incoming.local_id;
        incoming.deleted = true;
        incoming.save(bond).await?;

        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        bond: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        tracing::info!("Restoring block for {}", action.email);

        let Some(incoming_id) = action.removed else {
            return Err(anyhow!("Missing incoming default ID for: {}", action.email).into());
        };

        let Some(mut incoming) = IncomingDefault::find_by_id(incoming_id, bond).await? else {
            return Err(anyhow!("Missing incoming default for: {}", action.email).into());
        };

        incoming.deleted = false;
        incoming.save(bond).await?;

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
        tracing::info!("Unblocking {}", action.email);

        let Some(local_removed_id) = action.removed else {
            tracing::error!("Missing incoming default ID for: {}", action.email);
            return Ok(());
        };

        let ctx = self.ctx.upgrade().ok_or(MailActionError::LostContext)?;
        let mut tether = ctx.user_stash().connection();

        let Some(incoming) = IncomingDefault::find_by_id(local_removed_id, &tether).await? else {
            return Err(anyhow!("Missing incoming default for: {}", action.email).into());
        };

        if let Some(id) = incoming.remote_id.as_ref() {
            ctx.session().delete_incoming_default(id).await?;
        }

        tether
            .write_tx::<_, _, <Self::Action as Action<UserDb>>::Error>(async |tx| {
                incoming.delete(tx).await?;
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
