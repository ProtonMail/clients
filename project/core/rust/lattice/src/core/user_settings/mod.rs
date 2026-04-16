mod get_core_settings_2fa_register;
pub use self::get_core_settings_2fa_register::*;
mod get_user_settings;
pub use self::get_user_settings::*;
mod post_settings_2fa_register;
pub use self::post_settings_2fa_register::*;
mod post_device_recovery_secret;
pub use self::post_device_recovery_secret::*;
mod put_settings_device_recovery;
pub use self::put_settings_device_recovery::*;

use super::LtCoreU2FKey;
use crate::Sensitive;
use crate::auth::{LtAuthFidoKey, LtAuthPasswordMode, LtAuthTwoFactorMethod};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePasswordSettings {
    pub mode: LtAuthPasswordMode,
    #[cfg_attr(feature = "serde", serde(default))]
    pub expiration_time: Option<i64>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreTwoFactorSettings {
    pub enabled: LtAuthTwoFactorMethod,
    pub allowed: LtAuthTwoFactorMethod,
    #[cfg_attr(feature = "serde", serde(default))]
    pub expiration_time: Option<i64>,
    #[cfg_attr(feature = "serde", serde(rename = "U2FKeys", default))]
    pub u2f_keys: Vec<LtCoreU2FKey>,
    pub registered_keys: Sensitive<Vec<LtAuthFidoKey>>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreUserSettings {
    #[cfg_attr(feature = "serde", serde(default))]
    pub email: Option<LtCoreContactSettings>,
    pub password: LtCorePasswordSettings,
    #[cfg_attr(feature = "serde", serde(default))]
    pub phone: Option<LtCoreContactSettings>,
    #[cfg_attr(feature = "serde", serde(rename = "2FA"))]
    pub tfa: LtCoreTwoFactorSettings,
    #[cfg_attr(feature = "serde", serde(default))]
    pub news: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub locale: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub log_auth: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub density: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub week_start: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub date_format: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub time_format: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub early_access: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub flags: Option<LtCoreUserFlags>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub referral: Option<LtCoreReferralSettings>,
    #[cfg_attr(
        feature = "serde",
        serde(
            default,
            rename = "DeviceRecovery",
            skip_serializing_if = "Option::is_none",
            with = "crate::helpers::bool_opt_int"
        )
    )]
    pub device_recovery: Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub telemetry: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub crash_reports: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub invoice_text: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub theme_type: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub welcome: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub welcome_flag: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub hide_side_panel: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub organization_policy: Option<LtCoreOrganizationPolicy>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub high_security: Option<LtCoreHighSecurity>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub session_account_recovery: Option<i32>,
    #[cfg_attr(feature = "serde", serde(rename = "AIAssistantFlags", default))]
    pub ai_assistant_flags: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub used_client_flags: Option<i64>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub used_clients: Option<Vec<String>>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreContactSettings {
    #[cfg_attr(feature = "serde", serde(default))]
    pub value: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub status: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub notify: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub reset: Option<i32>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreUserFlags {
    #[cfg_attr(feature = "serde", serde(default))]
    pub welcomed: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub support_pgp_v6_keys: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub edm_opt_out: Option<i32>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreReferralSettings {
    #[cfg_attr(feature = "serde", serde(default))]
    pub link: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub eligible: Option<bool>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreOrganizationPolicy {
    #[cfg_attr(feature = "serde", serde(default))]
    pub enforced: Option<i32>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreHighSecurity {
    #[cfg_attr(feature = "serde", serde(default))]
    pub eligible: Option<i32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub value: Option<i32>,
}
