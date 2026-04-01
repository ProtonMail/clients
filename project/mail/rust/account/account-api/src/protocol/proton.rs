//! Proton API protocol types used by the account API.
//!
//! This module contains the types and traits from the Proton Core API that are
//! needed by the account flow. `mail-core-api` re-exports these types so that
//! all existing consumers of `mail_core_api::services::proton` continue to work
//! without change.

use mail_api_shared::ApiServiceResult;
use mail_muon::rest::auth::v4::{fido2, tfa::TFA};
use proton_crypto_account::keys::{APIPublicAddressKeys, AddressKeys, UserKeys};
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_default_from_null;
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{BoolFromInt, FromInto, serde_as};

// ---------------------------------------------------------------------------
// ID types
// ---------------------------------------------------------------------------

pub use mail_account_ids::{AddressId, PrivateEmail, SaltId, SessionId, UserId};
pub use mail_api_session::auth::PasswordMode;

// ---------------------------------------------------------------------------
// Auth types (formerly in core-api's services/proton/auth/)
// ---------------------------------------------------------------------------

/// `POST /auth/v4/info`
///
/// Initializes the SRP authentication process for the given user.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PostAuthInfoRequest {
    /// The username of the user to authenticate.
    pub username: String,
}

/// The response from a `POST /auth/v4/info` request.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PostAuthInfoResponse {
    /// The SRP session ID.
    #[serde(rename = "SRPSession")]
    pub session: String,

    /// The SRP version used by the server.
    pub version: u8,

    /// The user's salt.
    pub salt: String,

    /// The server's SRP modulus.
    pub modulus: String,

    /// The server's SRP ephemeral.
    pub server_ephemeral: String,

    /// The user's 2FA info (only if already logged in).
    #[serde(default)]
    #[serde(rename = "2FA")]
    pub tfa: Option<TFA>,
}

impl PostAuthInfoResponse {
    /// Returns FIDO2 keys and auth options.
    #[must_use]
    pub fn fido_details(&self) -> Option<fido2::Response> {
        self.tfa.as_ref()?.fido_details()
    }
}

impl Clone for PostAuthInfoResponse {
    fn clone(&self) -> Self {
        serde_json::to_value(self)
            .and_then(serde_json::from_value)
            .unwrap()
    }
}

/// The response containing the user's session UUID.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct GetSessionsUuidResponse {
    #[serde(rename = "UUID")]
    pub uuid: String,
}

#[allow(async_fn_in_trait)]
pub trait ProtonAuth {
    /// GET the user's session UUID.
    async fn get_sessions_uuid(&self) -> ApiServiceResult<GetSessionsUuidResponse>;

    /// POST auth info to initialize SRP authentication.
    async fn post_auth_info(
        &self,
        request: PostAuthInfoRequest,
    ) -> ApiServiceResult<PostAuthInfoResponse>;
}

// ---------------------------------------------------------------------------
// Metrics types
// ---------------------------------------------------------------------------

pub use mail_observability::{PostMetricsRequestData, PostMetricsRequestElement};

// ---------------------------------------------------------------------------
// Response data types (formerly in core-api's services/proton/core/response_data.rs)
// ---------------------------------------------------------------------------

/// TODO: Document this enum.
#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum AddressStatus {
    Disabled = 0,
    Enabled = 1,
    Deleting = 2,
}

/// TODO: Document this enum.
#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum AddressType {
    Original = 1,
    Alias = 2,
    Custom = 3,
    Premium = 4,
    External = 5,
}

/// TODO: Document this enum.
#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum UserMnemonicStatus {
    Disabled = 0,
    EnabledButNotSet = 1,
    EnabledNeedsReactivation = 2,
    EnabledAndSet = 3,
    Unknown = 4,
}

/// TODO: Document this enum.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum UserType {
    Proton = 1,
    Managed = 2,
    External = 3,
    CredentialLess = 4,
    Unknown(u8),
}

impl From<u8> for UserType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Proton,
            2 => Self::Managed,
            3 => Self::External,
            4 => Self::CredentialLess,
            v => Self::Unknown(v),
        }
    }
}

impl From<UserType> for u8 {
    fn from(value: UserType) -> Self {
        match value {
            UserType::Proton => 1,
            UserType::Managed => 2,
            UserType::External => 3,
            UserType::CredentialLess => 4,
            UserType::Unknown(v) => v,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Hash, PartialEq)]
#[repr(u32)]
pub enum Role {
    None = 0,
    Member = 1,
    Admin = 2,
    Unknown(u32),
}

impl From<u32> for Role {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Member,
            2 => Self::Admin,
            v => Self::Unknown(v),
        }
    }
}

impl From<Role> for u32 {
    fn from(value: Role) -> Self {
        match value {
            Role::None => 0,
            Role::Member => 1,
            Role::Admin => 2,
            Role::Unknown(v) => v,
        }
    }
}

/// Represents the delinquent state of the user.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize_repr, Serialize_repr, Eq)]
#[serde(rename_all = "PascalCase")]
#[repr(u32)]
pub enum DelinquentState {
    Paid = 0,
    Available = 1,
    Overdue = 2,
    Delinquent = 3,
    NotReceived = 4,
}

/// User flags returned from the API.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Flags {
    #[serde(rename = "has-temporary-password")]
    pub has_temporary_password: bool,

    #[serde(rename = "has-a-byoe-address")]
    pub has_a_byoe_address: bool,

    #[serde(rename = "no-login")]
    pub no_login: bool,

    #[serde(rename = "no-proton-address")]
    pub no_proton_address: bool,

    #[serde(rename = "onboard-checklist-storage-granted")]
    pub onboard_checklist_storage_granted: bool,

    pub protected: bool,

    #[serde(rename = "recovery-attempt")]
    pub recovery_attempt: bool,

    pub sso: bool,

    #[serde(rename = "test-account")]
    pub test_account: bool,
}

/// TODO: Document this struct.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ProductUsedSpace {
    pub calendar: i64,
    pub contact: i64,
    pub drive: i64,
    pub mail: i64,
    pub pass: i64,
}

/// TODO: Document this struct.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Password {
    pub mode: PasswordMode,
    pub expiration_time: Option<u64>,
}

/// Represents an API user.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct User {
    #[serde(rename = "ID")]
    pub id: UserId,

    pub create_time: u64,
    pub credit: i64,
    pub currency: String,
    pub delinquent: DelinquentState,
    pub display_name: Option<String>,
    pub email: String,
    pub flags: Flags,
    pub keys: UserKeys,
    pub max_space: i64,
    pub max_upload: i64,
    pub mnemonic_status: UserMnemonicStatus,
    pub name: Option<String>,

    #[serde_as(as = "BoolFromInt")]
    pub private: bool,

    pub product_used_space: ProductUsedSpace,

    #[serde_as(as = "FromInto<u32>")]
    pub role: Role,

    pub services: u32,
    pub subscribed: u32,

    #[serde_as(as = "BoolFromInt")]
    pub to_migrate: bool,

    pub used_space: i64,

    #[serde(rename = "Type")]
    #[serde_as(as = "FromInto<u8>")]
    pub user_type: UserType,
}

/// TODO: Document this struct.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[repr(transparent)]
pub struct AddressFlags(pub u32);

/// TODO: Document this struct.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct AddressSignedKeyList {
    pub data: Option<String>,

    #[serde(rename = "ExpectedMinEpochID")]
    pub expected_min_epoch_id: Option<u64>,

    #[serde(rename = "MaxEpochID")]
    pub max_epoch_id: Option<u64>,

    #[serde(rename = "MinEpochID")]
    pub min_epoch_id: Option<u64>,

    pub obsolescence_token: Option<String>,
    pub revision: u64,
    pub signature: Option<String>,
}

/// TODO: Document this struct.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct Address {
    #[serde(rename = "ID")]
    pub id: AddressId,

    #[serde(rename = "Type")]
    pub address_type: AddressType,

    pub catch_all: bool,
    pub display_name: String,

    #[serde(rename = "DomainID")]
    pub domain_id: Option<String>,

    pub email: String,
    pub keys: AddressKeys,
    pub order: u32,

    #[serde(rename = "ProtonMX")]
    pub proton_mx: bool,

    #[serde_as(as = "BoolFromInt")]
    pub receive: bool,

    #[serde_as(as = "BoolFromInt")]
    pub send: bool,

    pub signature: String,

    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub signed_key_list: AddressSignedKeyList,

    pub status: AddressStatus,
    pub flags: AddressFlags,
}

// ---------------------------------------------------------------------------
// Response types (formerly in core-api's services/proton/core/responses.rs)
// ---------------------------------------------------------------------------

/// The response containing addresses.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct GetAddressesResponse {
    pub addresses: Vec<Address>,
}

/// The response containing a single address.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct GetAddressResponse {
    pub address: Address,
}

/// The response containing a user.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct GetUsersResponse {
    pub user: User,
}

/// The response containing key salts.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct GetKeysSaltsResponse {
    pub key_salts: Vec<Salt>,
}

/// A key salt entry.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Salt {
    #[serde(rename = "ID")]
    pub id: SaltId,

    pub key_salt: Option<String>,
}

/// The response containing user settings.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct GetSettingsResponse {
    pub user_settings: UserSettings,
}

// ---------------------------------------------------------------------------
// UserSettings and supporting types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum DateFormat {
    Default = 0,
    DdMmYyyy = 1,
    MmDdYyyy = 2,
    YyyyMmDd = 3,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Density {
    Comfortable = 0,
    Compact = 1,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum LogAuth {
    Disabled = 0,
    Basic = 1,
    Advanced = 2,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum TimeFormat {
    Default = 0,
    H24 = 1,
    H12 = 2,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum WeekStart {
    Default = 0,
    Monday = 1,
    Saturday = 6,
    Sunday = 7,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Email {
    pub notify: u8,
    pub reset: u8,
    pub status: u8,
    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Phone {
    pub notify: u8,
    pub reset: u8,
    pub status: u8,
    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Referral {
    pub eligible: bool,
    pub link: String,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct SettingsFlags {
    #[serde_as(as = "BoolFromInt")]
    pub welcomed: bool,
    #[serde_as(as = "BoolFromInt")]
    pub edm_opt_out: bool,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct HighSecurity {
    #[serde_as(as = "BoolFromInt")]
    pub eligible: bool,
    #[serde_as(as = "BoolFromInt")]
    pub value: bool,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum TfaStatus {
    None = 0,
    Totp = 1,
    Fido2 = 2,
    TotpOrFido2 = 3,
}

impl From<TfaStatus> for mail_api_session::auth_mode::TfaStatus {
    fn from(value: TfaStatus) -> Self {
        match value {
            TfaStatus::None => Self::None,
            TfaStatus::Totp => Self::Totp,
            TfaStatus::Fido2 => Self::Fido2,
            TfaStatus::TotpOrFido2 => Self::TotpOrFido2,
        }
    }
}

impl From<mail_api_session::auth_mode::TfaStatus> for TfaStatus {
    fn from(value: mail_api_session::auth_mode::TfaStatus) -> Self {
        match value {
            mail_api_session::auth_mode::TfaStatus::None => Self::None,
            mail_api_session::auth_mode::TfaStatus::Totp => Self::Totp,
            mail_api_session::auth_mode::TfaStatus::Fido2 => Self::Fido2,
            mail_api_session::auth_mode::TfaStatus::TotpOrFido2 => Self::TotpOrFido2,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct FidoKey {
    pub attestation_format: String,
    #[serde(rename = "CredentialID")]
    pub credential_id: Vec<i32>,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TwoFa {
    pub allowed: TfaStatus,
    pub enabled: TfaStatus,
    pub expiration_time: Option<u64>,
    #[serde(default)]
    pub registered_keys: Vec<FidoKey>,
}

/// User settings returned from `GET /core/v4/settings`.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct UserSettings {
    #[serde_as(as = "BoolFromInt")]
    pub crash_reports: bool,

    pub date_format: DateFormat,

    #[serde_as(as = "BoolFromInt")]
    pub device_recovery: bool,

    pub density: Density,

    #[serde_as(as = "BoolFromInt")]
    pub early_access: bool,

    pub email: Email,
    pub flags: SettingsFlags,

    #[serde_as(as = "BoolFromInt")]
    pub hide_side_panel: bool,

    pub high_security: HighSecurity,
    pub invoice_text: String,
    pub locale: String,
    pub log_auth: LogAuth,
    pub news: u32,
    pub password: Password,
    pub phone: Phone,
    pub referral: Option<Referral>,

    #[serde_as(as = "BoolFromInt")]
    pub session_account_recovery: bool,

    #[serde_as(as = "BoolFromInt")]
    pub telemetry: bool,

    pub time_format: TimeFormat,

    #[serde(rename = "2FA")]
    pub two_factor_auth: TwoFa,

    pub week_start: WeekStart,

    #[serde_as(as = "BoolFromInt")]
    pub welcome: bool,
}

// ---------------------------------------------------------------------------
// Request types for ProtonAccount methods
// ---------------------------------------------------------------------------

/// Parameters for getting Captcha details.
#[serde_as]
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetCaptchaOptions {
    #[serde_as(as = "BoolFromInt")]
    pub force_web_messaging: bool,
    pub token: String,
}

/// Parameters for getting all keys.
#[serde_as]
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetKeysAllOptions {
    pub email: PrivateEmail,
    #[serde_as(as = "Option<BoolFromInt>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_only: Option<bool>,
}

// ---------------------------------------------------------------------------
// ProtonAccount trait
// ---------------------------------------------------------------------------

#[allow(async_fn_in_trait)]
pub trait ProtonAccount {
    async fn get_addresses(&self) -> ApiServiceResult<GetAddressesResponse>;

    async fn get_address_by_id(&self, id: AddressId) -> ApiServiceResult<GetAddressResponse>;

    async fn get_captcha(&self, options: GetCaptchaOptions) -> ApiServiceResult<String>;

    async fn get_keys_all(
        &self,
        options: GetKeysAllOptions,
    ) -> ApiServiceResult<APIPublicAddressKeys>;

    async fn get_keys_salts(&self) -> ApiServiceResult<GetKeysSaltsResponse>;

    async fn get_settings(&self) -> ApiServiceResult<GetSettingsResponse>;

    async fn get_users(&self) -> ApiServiceResult<GetUsersResponse>;
}
