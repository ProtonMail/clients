//! Core HTTP API v4 contracts.
//! [`LtAuthDevice`] and [`LtAuthDeviceState`] are re-exported from [`crate::auth::devices`] for use next to
//! other Core DTOs; the canonical import path is `lattice::auth::devices` when you only need those types.

pub mod account_enums;
pub mod addresses;
pub mod get_core_address;
pub mod get_core_addresses;
pub mod get_core_domains_available;
pub mod get_domain;
pub mod get_domains;
pub mod get_events;
pub mod get_keys_all;
pub mod get_members;
pub mod get_members_me_unprivatize;
pub mod get_organization_settings;
pub mod get_organizations;
pub mod get_organizations_keys;
pub mod get_organizations_keys_signature;
pub mod get_organizations_logo;
pub mod get_tests_ping;
pub mod ids;
pub mod keys;
pub use keys::post_keys_setup;
pub mod members;
pub mod post_domains;
pub mod post_members_keys_unprivatize;
pub mod post_members_saml;
pub mod post_members_unprivatize;
pub mod post_saml_setup_fields;
pub mod post_validate_email;
pub mod post_validate_phone;
pub mod put_core_address;
pub mod put_domain_flags;
pub mod put_keys_private;
pub mod put_organizations_keys_signature;
pub mod put_users_password;
pub mod unpriv_types;
pub mod user;
pub mod user_settings;

pub use crate::auth::devices::{LtAuthDevice, LtAuthDeviceState};
pub use account_enums::{
    LtCoreDomainVerifyState, LtCoreMemberOrgKeyStatus, LtCoreMemberState, LtCoreSsoType,
};
pub use addresses::{LtCoreAddressesListQuery, LtCoreAddressesListRes};
pub use get_members::{LtCoreMemberListAddress, LtCoreMemberListUnprivatization};
pub use ids::{LtCoreAuthDeviceId, LtCoreDomainId, LtCoreMemberEncId};
pub use members::addresses::LtCoreGetMembersMemberIDAddressesReq;
pub use members::devices::{
    LtCoreDeleteMembersDeviceReq, LtCoreDeleteMembersDevicesReq, LtCoreGetMembersDevicesPendingReq,
    LtCoreGetMembersDevicesPendingRes, LtCoreGetMembersDevicesReq, LtCoreGetMembersDevicesRes,
    LtCorePostMembersDevicesResetBody, LtCorePostMembersDevicesResetReq,
    LtCorePutMembersDevicesRejectReq, LtCoreResetAuthDevicesUserKey,
};
pub use post_members_keys_unprivatize::{
    LtCorePostMembersKeysUnprivatizeBody, LtCorePostMembersKeysUnprivatizeReq,
    LtCoreUnprivatizeAddressKey, LtCoreUnprivatizeOrganizationKeyActivation,
    LtCoreUnprivatizeUserKey,
};
pub use unpriv_types::{
    LtCoreUnprivActivationToken, LtCoreUnprivArmoredPrivateKey, LtCoreUnprivInvitationData,
    LtCoreUnprivInvitationSignature, LtCoreUnprivOrgKeyFingerprintSignature,
    LtCoreUnprivPgpPublicKey, LtCoreUnprivState,
};

use derive_more::{From, Into};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};

use crate::Sensitive;
use crate::auth::{LtAuthAddressId, LtAuthUserId};
use crate::core::keys::LtCoreSensitiveAddressKeys;

/// Async user initialization flag
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum LtCoreAsyncUserInitialization {
    Other = 0,
    CalledByClient = 1,
}

/// Represents a signed key with its data and signature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreSignedKeyList {
    /// JSON-encoded content of the SAL
    pub data: Sensitive<String>,

    /// The armored signature over the JSON-serialized data with the primary user key
    pub signature: Sensitive<String>,
}

impl From<proton_crypto_account::keys::LocalSignedKeyList> for LtCoreSignedKeyList {
    fn from(skl: proton_crypto_account::keys::LocalSignedKeyList) -> Self {
        Self {
            data: Sensitive::new(skl.data.to_string()),
            signature: Sensitive::new(skl.signature.to_string()),
        }
    }
}

/// Represents an address key input for key setup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreAddressKeyInput {
    /// The address ID.
    #[serde(rename = "AddressID")]
    pub address_id: LtAuthAddressId,

    /// The private key for the address.
    pub private_key: Sensitive<String>,

    pub primary: u8,

    /// The token associated with the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<Sensitive<String>>,

    /// The signature of the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Sensitive<String>>,

    /// Signed key list
    pub signed_key_list: LtCoreSignedKeyList,

    #[serde(default)]
    pub revision: i32,
}

/// The address of a user (copied from `proton-api-core`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct LtCoreAddress {
    #[serde(rename = "ID")]
    pub id: LtAuthAddressId,

    #[serde(rename = "Type")]
    pub address_type: LtCoreAddressType,

    pub catch_all: bool,

    pub display_name: Option<String>,

    #[serde(rename = "DomainID")]
    pub domain_id: Option<String>,

    pub email: String,

    pub keys: LtCoreSensitiveAddressKeys,

    pub order: u32,

    #[serde(rename = "ProtonMX")]
    pub proton_mx: bool,

    #[serde(with = "crate::helpers::bool_int")]
    pub receive: bool,

    #[serde(with = "crate::helpers::bool_int")]
    pub send: bool,

    pub signature: Option<String>,

    pub signed_key_list: Option<LtCoreAddressSignedKeyList>,

    pub status: LtCoreAddressStatus,

    #[serde(default)]
    pub flags: LtCoreAddressFlags,
}

/// Address-level bit flags returned by the API.
#[derive(From, Into)]
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize
)]
pub struct LtCoreAddressFlags(i32);

bitflags::bitflags! {
    impl LtCoreAddressFlags: i32 {
        const WhitelistedSpam = 1 << 0;
        const WhitelistedRate = 1 << 1;
        #[deprecated]
        const Starred = 1 << 2;
        const DomainClaimed = 1 << 3;
        const DisableE2EE = 1 << 4;
        const DisableExpectedSigned = 1 << 5;
        const BYOE = 1 << 6;
        const UsernameReclaimed = 1 << 7;
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreAddressSignedKeyList {
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

/// Represents the status of an address.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    TryFromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize
)]
#[repr(u8)]
#[serde(into = "u8", try_from = "u8")]
pub enum LtCoreAddressStatus {
    /// The address is disabled.
    Disabled = 0,

    /// The address is enabled.
    Enabled = 1,

    /// The address is in the process of being deleted.
    Deleting = 2,
}

/// This enum defines different categories of addresses with assigned integer values.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    TryFromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize
)]
#[repr(i32)]
#[serde(into = "i32", try_from = "i32")]
pub enum LtCoreAddressType {
    /// An initial type of address.
    Original = 1,

    /// A secondary or alternate address.
    Alias = 2,

    /// A tailored or unique address.
    Custom = 3,

    /// An enhanced or special address.
    Premium = 4,

    /// An address from an outside source.
    External = 5,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreU2FKey {
    pub label: String,
    pub key_handle: String,
    #[serde(default)]
    pub compromised: Option<i32>,
}

/// One `AuthDevices` event row (nested shape).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthDeviceEvent {
    #[serde(rename = "ID")]
    pub id: String,

    pub action: LtCoreEventAction,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_device: Option<LtAuthDevice>,
}

/// One `MemberAuthDevices` event row (nested shape).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreMemberAuthDeviceEvent {
    #[serde(rename = "ID")]
    pub id: String,

    pub action: LtCoreEventAction,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_auth_device: Option<LtAuthDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreEvents {
    #[serde(default)]
    pub users: Vec<LtCoreEventItem<LtAuthUserId>>,

    #[serde(default)]
    pub user_settings: Vec<LtCoreEventItem<LtAuthUserId>>,

    #[serde(default)]
    pub addresses: Vec<LtCoreEventItem<LtAuthAddressId>>,

    #[serde(default)]
    pub auth_devices: Vec<LtAuthDeviceEvent>,

    #[serde(default)]
    pub member_auth_devices: Vec<LtCoreMemberAuthDeviceEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreEventItem<Id> {
    #[serde(rename = "ID")]
    pub id: Id,

    pub action: LtCoreEventAction,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[derive(PartialEq, Eq, Hash)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[serde(into = "u8", try_from = "u8")]
pub enum LtCoreEventAction {
    Delete = 0,
    Create = 1,
    Update = 2,
    UpdateFlags = 3,
}

impl<T: Clone + PartialEq + Eq> LtCoreEventItem<T> {
    pub fn into<E: From<T> + Clone + PartialEq + Eq>(self) -> LtCoreEventItem<E> {
        LtCoreEventItem {
            id: self.id.into(),
            action: self.action,
        }
    }
}
