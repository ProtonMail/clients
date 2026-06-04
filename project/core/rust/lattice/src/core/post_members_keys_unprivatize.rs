//! `POST /core/v4/members/{id}/keys/unprivatize` — execute unprivatization (distinct from admin request at `POST .../unprivatize`).

use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive};

use super::ids::LtCoreMemberEncId;

/// Path member id + JSON body.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCorePostMembersKeysUnprivatizeReq {
    pub member_id: LtCoreMemberEncId,
    pub body: LtCorePostMembersKeysUnprivatizeBody,
}

/// `UnprivatizeMemberInput` body.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePostMembersKeysUnprivatizeBody {
    pub user_keys: Vec<LtCoreUnprivatizeUserKey>,
    pub address_keys: Vec<LtCoreUnprivatizeAddressKey>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub organization_key_activation: Option<LtCoreUnprivatizeOrganizationKeyActivation>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreUnprivatizeUserKey {
    pub org_private_key: Sensitive<String>,
    pub org_token: Sensitive<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreUnprivatizeAddressKey {
    #[cfg_attr(feature = "serde", serde(rename = "AddressKeyID"))]
    pub address_key_id: String,
    pub org_token_key_packet: Sensitive<String>,
    pub org_signature: Sensitive<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreUnprivatizeOrganizationKeyActivation {
    pub token_key_packet: Sensitive<String>,
    pub signature: Sensitive<String>,
}

impl LtContract for LtCorePostMembersKeysUnprivatizeReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<&'a LtCorePostMembersKeysUnprivatizeBody>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(&self.body)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/members/{}/keys/unprivatize",
            self.member_id
        )))
    }
}

impl AuthReq for LtCorePostMembersKeysUnprivatizeReq {}
