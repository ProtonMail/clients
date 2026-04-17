use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON, Method};

use super::{LtCoreDomainId, post_domains::LtCoreDomainOutput};

/// Request to update domain flags
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCorePutDomainsReq {
    /// The domain ID (enc_id in path)
    #[cfg_attr(feature = "serde", serde(skip))]
    pub domain_id: LtCoreDomainId,

    /// True if this domain is allowed for Mail usage
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub allowed_for_mail: Option<bool>,

    /// True if this domain is allowed for SSO usage
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", rename = "AllowedForSSO")
    )]
    pub allowed_for_sso: Option<bool>,
}

/// Response from the update domain flags endpoint
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCorePutDomainsRes {
    /// The updated domain details
    pub domain: LtCoreDomainOutput,
}

impl LtContract for LtCorePutDomainsReq {
    type Response = LtSlimAPIJSON<LtCorePutDomainsRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/core/v4/domains/{}", self.domain_id)))
    }
}

impl AuthReq for LtCorePutDomainsReq {}
