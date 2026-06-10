use crate::actions::MailActionError;
use crate::models::MailSettings;
use mail_action_queue::action::{Action, ActionId, DefaultVersionConverter, Handler, Type};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api::services::proton::ProtonMail;
use mail_api::services::proton::request_data::PutMailCategoryViewRequest;
use mail_core_api::session::Session;
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::{RunTransaction, WriteTx};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateCategoryView {
    pub enabled: bool,
    old_enabled: Option<bool>,
}

impl UpdateCategoryView {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            old_enabled: None,
        }
    }
}

impl Action<UserDb> for UpdateCategoryView {
    const TYPE: Type = Type("update_category_view");
    const VERSION: u32 = 1;
    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = UpdateCategoryViewHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailActionError;
}

pub struct UpdateCategoryViewHandler {
    pub api: Session,
}

impl Handler<UserDb> for UpdateCategoryViewHandler {
    type Action = UpdateCategoryView;

    async fn apply_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        bond: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        let mut mail_settings = match MailSettings::get(bond.tether()).await? {
            Some(ms) => ms,
            None => {
                tracing::warn!("Failed to get mail settings");
                MailSettings::default()
            }
        };

        action.old_enabled = Some(mail_settings.mail_category_view);
        mail_settings.mail_category_view = action.enabled;
        mail_settings.save(bond).await?;

        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        bond: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        let mut mail_settings = match MailSettings::get(bond.tether()).await? {
            Some(ms) => ms,
            None => {
                tracing::warn!("Failed to get mail settings");
                MailSettings::default()
            }
        };
        if let Some(old_enabled) = action.old_enabled {
            mail_settings.mail_category_view = old_enabled;
        }
        mail_settings.save(bond).await?;

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
        let request = PutMailCategoryViewRequest {
            mail_category_view: action.enabled,
        };

        let _response = self.api.put_mail_category_view(request).await?;

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
