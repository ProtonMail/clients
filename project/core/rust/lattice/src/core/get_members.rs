use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Sensitive};

use super::account_enums::{LtCoreMemberOrgKeyStatus, LtCoreMemberState};
use super::ids::LtCoreMemberEncId;
use super::unpriv_types::{
    LtCoreUnprivActivationToken, LtCoreUnprivArmoredPrivateKey, LtCoreUnprivInvitationData,
    LtCoreUnprivInvitationSignature, LtCoreUnprivState,
};
use super::user::LtCoreMemberRole;

/// Request to list all members of the authenticated user's organization
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct LtCoreGetMembersReq;

/// Response containing organization members
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCoreGetMembersRes {
    pub members: Vec<LtCoreMemberInfo>,
}

/// Member row as returned by core members APIs (`MemberInfo` in OpenAPI).
///
/// Definition: `apps/Account/app/Dto/MemberInfo.php`. JSON keys mostly PascalCase; `2faStatus` is lower camel.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCoreMemberInfo {
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: LtCoreMemberEncId,

    pub role: LtCoreMemberRole,

    pub state: LtCoreMemberState,

    pub private: i32,

    #[cfg_attr(feature = "serde", serde(rename = "Type"))]
    pub member_type: i32,

    pub max_space: i64,

    #[cfg_attr(feature = "serde", serde(rename = "MaxVPN"))]
    pub max_vpn: i32,

    pub name: String,

    pub used_space: i64,

    #[cfg_attr(feature = "serde", serde(rename = "Self"))]
    pub is_self: i32,

    #[cfg_attr(feature = "serde", serde(rename = "ToMigrate"))]
    pub to_migrate: i32,

    #[cfg_attr(feature = "serde", serde(rename = "BrokenSKL"))]
    pub broken_skl: i32,

    pub subscriber: i32,

    #[cfg_attr(feature = "serde", serde(rename = "SSO"))]
    pub sso: i32,

    #[cfg_attr(feature = "serde", serde(default))]
    pub two_factor_required_time: Option<i64>,

    #[cfg_attr(feature = "serde", serde(rename = "2faStatus"))]
    pub tfa_status: i32,

    pub keys: Vec<String>,

    #[cfg_attr(feature = "serde", serde(default))]
    pub public_key: Option<String>,

    /// Bitmask; see `bundles/AccountBundle/src/Organization/MemberPermission.php`.
    pub permissions: i32,

    #[cfg_attr(feature = "serde", serde(rename = "AccessToOrgKey"))]
    pub access_to_org_key: LtCoreMemberOrgKeyStatus,

    #[cfg_attr(feature = "serde", serde(rename = "NumAI"))]
    pub num_ai: i32,

    /// Unprivatization payload from `MagicLinkService::getUnprivatizationInfoForMember`: `null` or one object.
    ///
    /// Definition: `bundles/AccountInternalBundle/src/Application/Organization/MagicLinkService.php`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub unprivatization: Option<LtCoreMemberListUnprivatization>,

    #[cfg_attr(feature = "serde", serde(rename = "NumLumo"))]
    pub num_lumo: Option<i32>,

    /// Address rows included on member list responses (not always present in older OpenAPI snapshots).
    #[cfg_attr(feature = "serde", serde(default))]
    pub addresses: Vec<LtCoreMemberListAddress>,
}

/// One address entry under `Addresses` on [`LtCoreMemberInfo`].
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCoreMemberListAddress {
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: String,

    pub email: String,

    pub status: i32,

    #[cfg_attr(feature = "serde", serde(rename = "Type"))]
    pub address_type: i32,

    pub permissions: i32,
}

/// Embedded `Unprivatization` object on a member list row (non-invited members).
///
/// Keys match the associative array from `MagicLinkService::getUnprivatizationInfoForMember` (PascalCase in JSON).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCoreMemberListUnprivatization {
    #[cfg_attr(feature = "serde", serde(default))]
    pub state: Option<LtCoreUnprivState>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub invitation_data: Option<LtCoreUnprivInvitationData>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub invitation_signature: Option<LtCoreUnprivInvitationSignature>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub invitation_email: Option<String>,
    /// First split key; duplicate of `private_keys[0]` when both are set (Account).
    #[cfg_attr(feature = "serde", serde(default))]
    pub private_key: Option<LtCoreUnprivArmoredPrivateKey>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub private_keys: Option<Vec<Sensitive<String>>>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub activation_token: Option<LtCoreUnprivActivationToken>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub private_intent: Option<bool>,
}

impl LtContract for LtCoreGetMembersReq {
    type Response = LtSlimAPIJSON<LtCoreGetMembersRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/members"))
    }
}

impl AuthReq for LtCoreGetMembersReq {}
