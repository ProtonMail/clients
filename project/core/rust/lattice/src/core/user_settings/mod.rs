use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePasswordSettings {
    pub mode: LtAuthPasswordMode,
    #[serde(default)]
    pub expiration_time: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreTwoFactorSettings {
    pub enabled: LtAuthTwoFactorMethod,
    pub allowed: LtAuthTwoFactorMethod,
    #[serde(default)]
    pub expiration_time: Option<i64>,
    #[serde(rename = "U2FKeys", default)]
    pub u2f_keys: Vec<LtCoreU2FKey>,
    pub registered_keys: Sensitive<Vec<LtAuthFidoKey>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreUserSettings {
    #[serde(default)]
    pub email: Option<LtCoreContactSettings>,
    pub password: LtCorePasswordSettings,
    #[serde(default)]
    pub phone: Option<LtCoreContactSettings>,
    #[serde(rename = "2FA")]
    pub tfa: LtCoreTwoFactorSettings,
    #[serde(default)]
    pub news: Option<i32>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub log_auth: Option<i32>,
    #[serde(default)]
    pub density: Option<i32>,
    #[serde(default)]
    pub week_start: Option<i32>,
    #[serde(default)]
    pub date_format: Option<i32>,
    #[serde(default)]
    pub time_format: Option<i32>,
    #[serde(default)]
    pub early_access: Option<i32>,
    #[serde(default)]
    pub flags: Option<LtCoreUserFlags>,
    #[serde(default)]
    pub referral: Option<LtCoreReferralSettings>,
    #[serde(
        default,
        rename = "DeviceRecovery",
        skip_serializing_if = "Option::is_none",
        with = "crate::helpers::bool_opt_int"
    )]
    pub device_recovery: Option<bool>,
    #[serde(
        default,
        rename = "Telemetry",
        skip_serializing_if = "Option::is_none",
        with = "crate::helpers::bool_opt_int"
    )]
    pub telemetry: Option<bool>,
    #[serde(
        default,
        rename = "CrashReports",
        skip_serializing_if = "Option::is_none",
        with = "crate::helpers::bool_opt_int"
    )]
    pub crash_reports: Option<bool>,
    #[serde(default)]
    pub invoice_text: Option<String>,
    #[serde(default)]
    pub theme_type: Option<i32>,
    #[serde(default)]
    pub welcome: Option<i32>,
    #[serde(default)]
    pub welcome_flag: Option<i32>,
    #[serde(default)]
    pub hide_side_panel: Option<i32>,
    #[serde(default)]
    pub organization_policy: Option<LtCoreOrganizationPolicy>,
    #[serde(default)]
    pub high_security: Option<LtCoreHighSecurity>,
    #[serde(default)]
    pub session_account_recovery: Option<i32>,
    #[serde(rename = "AIAssistantFlags", default)]
    pub ai_assistant_flags: Option<i32>,
    #[serde(default)]
    pub used_client_flags: Option<i64>,
    #[serde(default)]
    pub used_clients: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreContactSettings {
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub notify: Option<i32>,
    #[serde(default)]
    pub reset: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreUserFlags {
    #[serde(default)]
    pub welcomed: Option<i32>,
    #[serde(default)]
    pub support_pgp_v6_keys: Option<i32>,
    #[serde(default)]
    pub edm_opt_out: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreReferralSettings {
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub eligible: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreOrganizationPolicy {
    #[serde(default)]
    pub enforced: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreHighSecurity {
    #[serde(default)]
    pub eligible: Option<i32>,
    #[serde(default)]
    pub value: Option<i32>,
}
