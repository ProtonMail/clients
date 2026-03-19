pub mod get_core_address;
pub mod get_core_addresses;
pub mod get_core_domains_available;
pub mod get_core_settings_2fa_register;
pub mod get_events;
pub mod get_key_salts;
pub mod get_settings;
pub mod get_tests_ping;
pub mod keys;
pub mod post_keys_setup;
pub mod post_settings_2fa_register;
pub mod post_validate_email;
pub mod post_validate_phone;
pub mod put_users_password;
pub mod user;

use derive_more::{From, Into};
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{
    Sensitive,
    auth::{
        LtAuthAddressId, LtAuthFidoKey, LtAuthPasswordMode, LtAuthTwoFactorMethod, LtAuthUserId,
    },
    core::keys::LtCoreSensitiveAddressKeys,
};

/// Async user initialization flag
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(into = "i32", try_from = "i32"))]
pub enum LtCoreAsyncUserInitialization {
    Other = 0,
    CalledByClient = 1,
}

/// Represents a signed key with its data and signature.
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreSignedKeyList {
    /// JSON-encoded content of the SAL
    pub data: Sensitive<String>,

    /// The armored signature over the JSON-serialized data with the primary user key
    pub signature: Sensitive<String>,
}

/// Represents an address key input for key setup.
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreAddressKeyInput {
    /// The address ID.
    #[cfg_attr(feature = "serde", serde(rename = "AddressID"))]
    pub address_id: String,

    /// The private key for the address.
    pub private_key: Sensitive<String>,

    pub primary: u8,

    /// The token associated with the key.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub token: Option<Sensitive<String>>,

    /// The signature of the key.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub signature: Option<Sensitive<String>>,

    /// Signed key list
    pub signed_key_list: LtCoreSignedKeyList,

    #[cfg_attr(feature = "serde", serde(default))]
    pub revision: i32,
}

/// The address of a user (copied from `proton-api-core`)
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
#[allow(clippy::struct_excessive_bools)]
pub struct LtCoreAddress {
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: LtAuthAddressId,

    #[cfg_attr(feature = "serde", serde(rename = "Type"))]
    pub address_type: LtCoreAddressType,

    pub catch_all: bool,

    pub display_name: Option<String>,

    #[cfg_attr(feature = "serde", serde(rename = "DomainID"))]
    pub domain_id: Option<String>,

    pub email: String,

    pub keys: LtCoreSensitiveAddressKeys,

    pub order: u32,

    #[cfg_attr(feature = "serde", serde(rename = "ProtonMX"))]
    pub proton_mx: bool,

    #[cfg_attr(feature = "serde", serde(with = "crate::helpers::bool_int"))]
    pub receive: bool,

    #[cfg_attr(feature = "serde", serde(with = "crate::helpers::bool_int"))]
    pub send: bool,

    pub signature: Option<String>,

    pub signed_key_list: Option<LtCoreAddressSignedKeyList>,

    pub status: LtCoreAddressStatus,

    #[cfg_attr(feature = "serde", serde(default))]
    pub flags: LtCoreAddressFlags,
}

/// Address-level bit flags returned by the API.
#[derive(From, Into)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
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
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreAddressSignedKeyList {
    pub data: Option<String>,

    #[cfg_attr(feature = "serde", serde(rename = "ExpectedMinEpochID"))]
    pub expected_min_epoch_id: Option<u64>,

    #[cfg_attr(feature = "serde", serde(rename = "MaxEpochID"))]
    pub max_epoch_id: Option<u64>,

    #[cfg_attr(feature = "serde", serde(rename = "MinEpochID"))]
    pub min_epoch_id: Option<u64>,

    pub obsolescence_token: Option<String>,

    pub revision: u64,

    pub signature: Option<String>,
}

/// Represents the status of an address.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    TryFromPrimitive,
    IntoPrimitive
)]
#[repr(u8)]
#[cfg_attr(feature = "serde", serde(into = "u8", try_from = "u8"))]
pub enum LtCoreAddressStatus {
    /// The address is disabled.
    Disabled = 0,

    /// The address is enabled.
    Enabled = 1,

    /// The address is in the process of being deleted.
    Deleting = 2,
}

/// This enum defines different categories of addresses with assigned integer values.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    TryFromPrimitive,
    IntoPrimitive
)]
#[repr(i32)]
#[cfg_attr(feature = "serde", serde(into = "i32", try_from = "i32"))]
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

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreUserSettings {
    pub password: LtCorePasswordSettings,
    #[cfg_attr(feature = "serde", serde(rename = "2FA"))]
    pub tfa: LtCoreTwoFactorSettings,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreTwoFactorSettings {
    pub enabled: LtAuthTwoFactorMethod,
    pub allowed: LtAuthTwoFactorMethod,
    pub registered_keys: Sensitive<Vec<LtAuthFidoKey>>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePasswordSettings {
    pub mode: LtAuthPasswordMode,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreEvents {
    #[cfg_attr(feature = "serde", serde(default))]
    pub users: Vec<LtCoreEventItem<LtAuthUserId>>,

    #[cfg_attr(feature = "serde", serde(default))]
    pub user_settings: Vec<LtCoreEventItem<LtAuthUserId>>,

    #[cfg_attr(feature = "serde", serde(default))]
    pub addresses: Vec<LtCoreEventItem<LtAuthAddressId>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LtCoreEventItem<Id> {
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: Id,

    #[cfg_attr(feature = "serde", serde(rename = "Action"))]
    pub action: LtCoreEventAction,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
#[derive(PartialEq, Eq, Hash)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "u8", try_from = "u8"))]
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
