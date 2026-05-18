use super::MailUserSession;
use super::datatypes::MailSettings;
use super::state::MailUserContextPtr;
use crate::errors::unexpected::UnexpectedError;
use crate::errors::{ProtonError, UserSessionError};
use crate::{LiveQueryCallback, WatchHandle, declare_live_query_tagger, uniffi_async};
use mail_common::models::{
    CustomSettings as RealCustomSettings, MailSettings as RealMailSettings,
    MobileSignatureStatus as RealMobileSignatureStatus,
};
use mail_common::{MailContextError, MailUserContext, ProtonMailError as RealProtonMailError};
use mail_core_common::models::ModelExtension;
use mail_uniffi_runtime::async_runtime;
use std::sync::Arc;
use tracing::instrument;
use uniffi::{Enum, Object, Record};

#[uniffi_export]
pub async fn mail_settings(ctx: &MailUserSession) -> Result<MailSettings, UserSessionError> {
    let mail_stash = ctx.user_stash()?;

    Ok(uniffi_async::<_, MailContextError, _>(async move {
        let tether = mail_stash.connection();

        Ok(RealMailSettings::get_or_default(&tether).await)
    })
    .await
    .unwrap_or_default()
    .into())
}

#[uniffi_export]
#[tracing::instrument(skip_all)]
pub fn mail_settings_sync(ctx: &MailUserSession) -> Result<MailSettings, UserSessionError> {
    let mail_stash = ctx.user_stash()?;

    Ok(async_runtime()
        .block_on(async move {
            let tether = mail_stash.connection();

            Ok::<_, MailContextError>(RealMailSettings::get_or_default(&tether).await)
        })
        .unwrap_or_default()
        .into())
}

#[derive(Clone, Record)]
pub struct SettingsWatcher {
    pub settings: MailSettings,
    pub watch_handle: Arc<WatchHandle>,
}

declare_live_query_tagger!(WatchMailSettingsMarker);

#[uniffi_export]
#[tracing::instrument(skip_all)]
pub async fn watch_mail_settings(
    ctx: &MailUserSession,
    callback: Box<dyn LiveQueryCallback>,
) -> Result<SettingsWatcher, UserSessionError> {
    let ctx = ctx.ctx()?;

    uniffi_async(async move {
        let mail_stash = ctx.user_stash();
        let tether = mail_stash.connection();

        let settings = RealMailSettings::all(&tether)
            .await?
            .pop()
            .unwrap_or_default()
            .into();

        let handle = RealMailSettings::watch(mail_stash).await?;
        let watcher = WatchMailSettingsMarker::watch_channel(&*ctx, handle, callback);

        Result::<_, RealProtonMailError>::Ok(SettingsWatcher {
            watch_handle: watcher,
            settings,
        })
    })
    .await
    .map_err(UserSessionError::from)
}

#[derive(Clone, Object)]
pub struct CustomSettings {
    ctx: MailUserContextPtr,
}

impl CustomSettings {
    fn ctx(&self) -> Result<Arc<MailUserContext>, ProtonError> {
        self.ctx
            .upgrade()
            .ok_or(ProtonError::Unexpected(UnexpectedError::Unknown))
    }
}

#[uniffi_export]
impl CustomSettings {
    #[instrument(skip_all)]
    pub async fn mobile_signature(&self) -> Result<MobileSignature, ProtonError> {
        let ctx = self.ctx()?;

        uniffi_async::<_, RealProtonMailError, _>(async move {
            let user = ctx.user().await?;
            let tether = ctx.user_stash().connection();
            let settings = RealCustomSettings::get_or_default(&tether).await?;
            let status = RealMobileSignatureStatus::new(&user, &settings);

            Ok(MobileSignature {
                body: settings.mobile_signature().to_owned(),
                status: status.into(),
            })
        })
        .await
        .map_err(Into::into)
    }

    #[instrument(skip_all)]
    pub async fn set_mobile_signature(&self, signature: String) -> Result<(), ProtonError> {
        let ctx = self.ctx()?;

        uniffi_async::<_, RealProtonMailError, _>(async move {
            RealCustomSettings::update_mobile_signature(&ctx, Some(signature)).await?;

            Ok(())
        })
        .await
        .map_err(ProtonError::from)
    }

    #[instrument(skip_all)]
    pub async fn set_mobile_signature_enabled(&self, enabled: bool) -> Result<(), ProtonError> {
        let ctx = self.ctx()?;

        uniffi_async::<_, RealProtonMailError, _>(async move {
            RealCustomSettings::update_mobile_signature_enabled(&ctx, Some(enabled)).await?;

            Ok(())
        })
        .await
        .map_err(ProtonError::from)
    }

    pub async fn swipe_to_adjacent_conversation(&self) -> Result<bool, ProtonError> {
        let ctx = self.ctx()?;

        uniffi_async::<_, RealProtonMailError, _>(async move {
            let tether = ctx.user_stash().connection();
            let settings = RealCustomSettings::get_or_default(&tether).await?;

            Ok(settings.swipe_to_adjacent_conversation())
        })
        .await
        .map_err(Into::into)
    }

    pub async fn set_swipe_to_adjacent_conversation(
        &self,
        enabled: bool,
    ) -> Result<(), ProtonError> {
        let ctx = self.ctx()?;

        uniffi_async::<_, RealProtonMailError, _>(async move {
            RealCustomSettings::update_swipe_to_adjacent_conversation(&ctx, Some(enabled)).await?;

            Ok(())
        })
        .await
        .map_err(ProtonError::from)
    }
}

#[derive(Clone, Record)]
pub struct MobileSignature {
    pub body: String,
    pub status: MobileSignatureStatus,
}

#[derive(Clone, Copy, Enum)]
pub enum MobileSignatureStatus {
    Enabled,
    Disabled,
    NeedsPaidVersion,
}

impl From<RealMobileSignatureStatus> for MobileSignatureStatus {
    fn from(value: RealMobileSignatureStatus) -> Self {
        use RealMobileSignatureStatus as Lhs;

        match value {
            Lhs::Enabled => Self::Enabled,
            Lhs::Disabled => Self::Disabled,
            Lhs::NeedsPaidVersion => Self::NeedsPaidVersion,
        }
    }
}

#[uniffi_export]
pub async fn update_next_message_on_move(
    ctx: &MailUserSession,
    enabled: bool,
) -> Result<(), UserSessionError> {
    let ctx = ctx.ctx()?;

    uniffi_async::<_, RealProtonMailError, _>(async move {
        RealMailSettings::action_update_next_message_on_move(&ctx.action_queue(), enabled).await?;

        Ok(())
    })
    .await
    .map_err(UserSessionError::from)
}

#[uniffi_export]
pub async fn update_category_view(
    ctx: &MailUserSession,
    enabled: bool,
) -> Result<(), UserSessionError> {
    let ctx = ctx.ctx()?;

    uniffi_async::<_, RealProtonMailError, _>(async move {
        RealMailSettings::action_update_next_message_on_move(&ctx.action_queue(), enabled).await?;

        Ok(())
    })
    .await
    .map_err(UserSessionError::from)
}

#[uniffi_export]
#[must_use]
pub fn custom_settings(ctx: &MailUserSession) -> Arc<CustomSettings> {
    Arc::new(CustomSettings { ctx: ctx.ptr() })
}
