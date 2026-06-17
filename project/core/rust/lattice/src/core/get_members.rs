use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::num::NonZeroU32;

use crate::auth::LtAuthAddressId;
use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtPaginable, LtSlimAPIJSON};

use super::account_enums::{LtCoreMemberOrgKeyStatus, LtCoreMemberState};
use super::ids::LtCoreMemberEncId;
use super::keys::LtCoreSensitiveUserKeys;
use super::unpriv_types::{
    LtCoreUnprivActivationToken, LtCoreUnprivArmoredPrivateKey, LtCoreUnprivInvitationData,
    LtCoreUnprivInvitationSignature, LtCoreUnprivState,
};
use super::user::LtCoreMemberRole;

/// Request to list all members of the authenticated user's organization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct LtCoreGetMembersReq;

/// Response containing organization members.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetMembersRes {
    pub members: Vec<LtCoreMemberInfo>,
    /// This will only be present if the request includes pagination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
}

/// Member row as returned by core members APIs (`MemberInfo` in OpenAPI).
///
/// Definition: `apps/Account/app/Dto/MemberInfo.php`. JSON keys mostly PascalCase; `2faStatus` is lower camel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreMemberInfo {
    #[serde(rename = "ID")]
    pub id: LtCoreMemberEncId,

    pub role: LtCoreMemberRole,

    pub state: LtCoreMemberState,

    pub private: i32,

    #[serde(rename = "Type")]
    pub member_type: i32,

    pub max_space: i64,

    #[serde(rename = "MaxVPN")]
    pub max_vpn: i32,

    pub name: String,

    pub used_space: i64,

    #[serde(rename = "Self")]
    pub is_self: i32,

    #[serde(rename = "ToMigrate")]
    pub to_migrate: i32,

    #[serde(rename = "BrokenSKL")]
    pub broken_skl: i32,

    pub subscriber: i32,

    #[serde(rename = "SSO")]
    pub sso: i32,

    #[serde(default)]
    pub two_factor_required_time: Option<i64>,

    #[serde(rename = "2faStatus")]
    pub tfa_status: i32,

    /// User keys from `UserKey::getCompleteInfo()` (armored material when present).
    pub keys: LtCoreSensitiveUserKeys,

    #[serde(default)]
    pub public_key: Option<String>,

    /// Bitmask; see `bundles/AccountBundle/src/Organization/MemberPermission.php`.
    pub permissions: i32,

    #[serde(rename = "AccessToOrgKey")]
    pub access_to_org_key: LtCoreMemberOrgKeyStatus,

    #[serde(rename = "NumAI")]
    pub num_ai: i32,

    /// Unprivatization payload from `MagicLinkService::getUnprivatizationInfoForMember`: `null` or one object.
    ///
    /// Definition: `bundles/AccountInternalBundle/src/Application/Organization/MagicLinkService.php`.
    #[serde(default)]
    pub unprivatization: Option<LtCoreMemberListUnprivatization>,

    #[serde(rename = "NumLumo")]
    pub num_lumo: Option<i32>,

    /// Address rows included on member list responses (`withAddress: true` on list members).
    #[serde(default)]
    pub addresses: Vec<LtCoreMemberListAddress>,
}

/// One address entry under `Addresses` on [`LtCoreMemberInfo`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreMemberListAddress {
    #[serde(rename = "ID")]
    pub id: LtAuthAddressId,

    pub email: String,

    pub status: i32,

    #[serde(rename = "Type")]
    pub address_type: i32,

    pub permissions: i32,
}

/// Embedded `Unprivatization` object on a member list row (non-invited members).
///
/// Keys match the associative array from `MagicLinkService::getUnprivatizationInfoForMember` (PascalCase in JSON).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreMemberListUnprivatization {
    #[serde(default)]
    pub state: Option<LtCoreUnprivState>,
    #[serde(default)]
    pub invitation_data: Option<LtCoreUnprivInvitationData>,
    #[serde(default)]
    pub invitation_signature: Option<LtCoreUnprivInvitationSignature>,
    #[serde(default)]
    pub invitation_email: Option<String>,
    /// First split key; duplicate of `private_keys[0]` when both are set (Account).
    #[serde(default)]
    pub private_key: Option<LtCoreUnprivArmoredPrivateKey>,
    #[serde(default)]
    pub private_keys: Option<Vec<LtCoreUnprivArmoredPrivateKey>>,
    #[serde(default)]
    pub activation_token: Option<LtCoreUnprivActivationToken>,
    #[serde(default)]
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

impl LtPaginable for LtCoreGetMembersReq {
    type Item = LtCoreMemberInfo;
    const MAX_PAGE_SIZE: NonZeroU32 = NonZeroU32::new(150).unwrap();

    fn page_items(res: LtSlimAPIJSON<LtCoreGetMembersRes>) -> (Option<u32>, Vec<LtCoreMemberInfo>) {
        (res.0.total, res.0.members)
    }
}

impl AuthReq for LtCoreGetMembersReq {}
