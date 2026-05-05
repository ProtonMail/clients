use std::borrow::Cow;

use crate::auth::LtAuthAddressId;
use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Sensitive};

use super::account_enums::LtCoreMemberOrgKeyStatus;

/// `GET /core/v4/organizations/keys` — org key material for the member (see
/// `GetOrganizationKeysOutput` / `GetOrganizationKeysQueryHandler`).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetOrganizationsKeysReq;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetOrganizationsKeysRes {
    /// Armored org private key (member’s copy); `null` when the member must not see the key.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub private_key: Option<Sensitive<String>>,

    /// Present when migrating org key.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub legacy_private_key: Option<Sensitive<String>>,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub public_key: Option<String>,

    /// PGP message (passwordless token packets), armored.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub token: Option<Sensitive<String>>,

    /// Armored signature over token secret.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub signature: Option<Sensitive<String>>,

    /// Encrypted id of the address that produced `signature` (if applicable).
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub signature_address: Option<LtAuthAddressId>,

    /// Encrypted address id.
    #[cfg_attr(
        feature = "serde",
        serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "EncryptionAddressID"
        )
    )]
    pub encryption_address_id: Option<LtAuthAddressId>,

    /// Armored sig over org key SHA-256 fingerprint.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub fingerprint_signature: Option<Sensitive<String>>,

    /// Encrypted id of the address for `fingerprint_signature` (if applicable).
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub fingerprint_signature_address: Option<LtAuthAddressId>,

    /// See `MemberOrgKeyStatus` in Account.
    pub access_to_org_key: LtCoreMemberOrgKeyStatus,

    pub passwordless: bool,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub parent_org_token: Option<Sensitive<String>>,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub parent_org_token_signature: Option<Sensitive<String>>,
}

impl LtContract for LtCoreGetOrganizationsKeysReq {
    type Response = LtSlimAPIJSON<LtCoreGetOrganizationsKeysRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/organizations/keys"))
    }
}

impl AuthReq for LtCoreGetOrganizationsKeysReq {}
