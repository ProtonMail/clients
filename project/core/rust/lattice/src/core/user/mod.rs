use serde::{Deserialize, Serialize};
pub mod get_users;
pub mod get_users_available;
pub mod get_users_available_external;
pub mod post_code;
pub mod post_users;
pub mod post_users_external;

use derive_more::{From, Into};
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{Sensitive, core::keys::LtCoreSensitiveUserKeys};

/// The type of account to create.
#[repr(u8)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub enum LtCoreCreateUserType {
    Normal = 1,

    #[deprecated]
    Username = 2,
}

/// Represents the SRP verifier data for authentication.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreSrpVerifier {
    /// The version of the authentication.
    pub version: u8,

    /// The modulus ID for authentication.
    #[serde(rename = "ModulusID")]
    pub modulus_id: String,

    /// The salt used in authentication.
    pub salt: Sensitive<String>,

    /// The verifier for authentication.
    pub verifier: Sensitive<String>,
}

/// Indicates whether the username should be parsed as a full email address.
#[repr(u8)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    Serialize,
    Deserialize
)]
#[serde(into = "u8", try_from = "u8")]
pub enum LtCoreParseDomain {
    /// The username is not a full email address (default).
    #[default]
    NoEmail = 0,
    /// The username is a full email address.
    FullEmail = 1,
}

/// Definition: bundles/AccountInternalBundle/src/Application/User/GetUserInfoQueryHandler.php
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreUser {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub currency: String,
    pub credit: i32,
    /// 1: Proton (full), 2: Managed, 3: External, 4: Credentialless
    #[serde(rename = "Type")]
    pub user_type: LtCoreUserType,
    pub create_time: i64,
    /// Max space (in bytes)
    pub max_space: i64,
    /// Max upload space (in bytes)
    pub max_upload: i64,
    /// Used space (in bytes)
    pub used_space: i64,
    pub max_base_space: Option<i64>,
    pub max_drive_space: Option<i64>,
    pub used_base_space: Option<i64>,
    pub used_drive_space: Option<i64>,
    pub product_used_space: LtCoreProductUsedSpace,
    /// 1 when the user's member has an AI seat, 0 otherwise
    #[serde(rename = "NumAI")]
    pub num_ai: i32,
    /// The number of lumo seats attributed to the user, 0 otherwise
    pub num_lumo: i32,
    /// Subscribed (bitmap): `1`: User has a mail subscription, `4`: User has a VPN subscription
    pub subscribed: LtCoreProductGroup,
    /// Activated services (bitmap): `1`: User has the mail product activated, `4`: User has the VPN activated
    pub services: LtCoreProductGroup,
    pub mnemonic_status: LtCoreMnemonicStatus,
    pub role: LtCoreMemberRole,
    #[serde(with = "crate::helpers::bool_int")]
    pub private: bool,
    pub delinquent: LtCoreDelinquentState,
    #[serde(with = "crate::helpers::bool_int")]
    pub billed: bool,
    pub keys: LtCoreSensitiveUserKeys,
    #[serde(with = "crate::helpers::bool_int")]
    pub to_migrate: bool,
    pub organization_private_key: Option<Sensitive<String>>,
    pub account_recovery: Option<LtCoreAccountRecoveryAttempt>,
    pub flags: LtCoreUserFlags,
    pub locked_flags: Option<LtCoreUserLockedFlag>,
}

/// Definition: bundles/AccountLegacyBundle/src/Model/UserUsage.php
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreProductUsedSpace {
    pub calendar: i64,
    pub contact: i64,
    pub drive: i64,
    pub mail: i64,
    pub pass: i64,
    pub lumo: i64,
}

/// Definition: apps/Account/app/Dto/AccountRecoveryAttempt.php
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreAccountRecoveryAttempt {
    /// 0 => None, 1 => Grace, 2 => Cancelled, 3 => Insecure, 4 => Expired
    pub state: LtCoreAccountRecoveryAttemptState,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    /// 0 => None, 1 => Cancelled, 2 => Authentication
    pub reason: Option<LtCoreAccountRecoveryAttemptCancellationReason>,
    /// The session ID that triggered the process
    #[serde(rename = "UID")]
    pub uid: String,
}

/// Definition: apps/Account/app/Enum/AccountRecoveryAttemptState.php
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum LtCoreAccountRecoveryAttemptState {
    Grace = 1,
    Cancelled = 2,
    Insecure = 3,
    Expired = 4,
}

/// Definition: apps/Account/app/Enum/AccountRecoveryAttemptCancellationReason.php
#[repr(u8)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
// Note: using u8 here to match the repr and the TryFrom implementation
#[serde(into = "u8", try_from = "u8")]
pub enum LtCoreAccountRecoveryAttemptCancellationReason {
    AbortedUi = 1,
    CancelledAuth = 2,
}

/// Definition: bundles/AccountBundle/src/User/UserFlags.php
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LtCoreUserFlags {
    pub protected: bool,
    pub drive_early_access: bool,
    pub onboard_checklist_storage_granted: bool,
    pub has_temporary_password: bool,
    pub test_account: bool,
    /// Prevent login (and thus 2FA check) for service accounts
    pub no_login: bool,
    pub recovery_attempt: bool,
    pub sso: bool,
    pub pass_lifetime: bool,
    pub pass_from_sl: bool,
    /// Whether the user has at least one bring-your-own-email address
    pub has_a_byoe_address: bool,
    // TODO(check): Those two flags aren't in the backend code how is that possible?
    /// User have no or only external addresses
    pub no_proton_address: bool,
    pub delegated_access: bool,
}

/// Definition: bundles/CoreBundle/src/Enum/ProductGroup.php
#[derive(From, Into)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreProductGroup(i32);

bitflags::bitflags! {
    impl LtCoreProductGroup: i32 {
        const Inbox = 1 << 0;
        const Drive = 1 << 1;
        const Vpn = 1 << 2;
        const Pass = 1 << 3;
        const Wallet = 1 << 4;
        const Neutron = 1 << 5;
        const Lumo = 1 << 6;
        const Authenticator = 1 << 7;
        const Meet = 1 << 8;
        const Docs = 1 << 9;
    }
}

/// Definition: apps/Account/app/Enum/UserLockedFlag.php
#[derive(From, Into)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreUserLockedFlag(i32);

bitflags::bitflags! {
    impl LtCoreUserLockedFlag: i32 {
        const BaseStorageExceeded = 1 << 0;
        const DriveStorageExceeded = 1 << 1;
        const PrimaryAdminOfOrgWithMembers = 1 << 2;
        const MemberOfOrgWithMembers = 1 << 3;
        const UserWithDomain = 1 << 4;
    }
}

/// Definition: bundles/AccountBundle/src/Enum/MnemonicStatus.php
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum LtCoreMnemonicStatus {
    /// Mnemonic has been opted-out in the settings
    Disabled = 0,
    /// Mnemonic is enabled but not set (requires user action)
    Enabled = 1,
    /// Mnemonic is set but in an old state or compromised and needs to be reset
    ///
    /// In this case the mnemonic can still be used to recover some old keys (partial recovery data is still
    /// available) but not to fully reset an account, since we do not have the guarantee of having the latest user
    /// key available.
    Outdated = 2,
    /// Mnemonic is OK
    Set = 3,
    /// User should be prompted to enable the mnemonic - alias of 1 for FE
    Prompt = 4,
}

/// Definition: bundles/AccountBundle/src/User/UserType.php
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum LtCoreUserType {
    /// Internal users
    Proton = 1,
    /// Sub-users
    Managed = 2,
    External = 3,
    /// Credential-less users
    Credentialless = 4,
}

/// Definition: bundles/AccountBundle/src/Organization/MemberRole.php
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum LtCoreMemberRole {
    None = 0,
    Member = 1,
    Admin = 2,
}

/// Definition: bundles/NewPaymentsBundle/src/ValueObject/DelinquentState.php
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum LtCoreDelinquentState {
    Paid = 0,
    Available = 1,
    Overdue = 2,
}
