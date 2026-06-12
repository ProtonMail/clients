use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

use super::{LtCoreDomainId, post_domains::LtCoreDomainOutput};

/// Request to update domain flags
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCorePutDomainsReq {
    /// The domain ID (enc_id in path)
    #[serde(skip)]
    pub domain_id: LtCoreDomainId,

    /// True if this domain is allowed for Mail usage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_for_mail: Option<bool>,

    /// True if this domain is allowed for SSO usage
    #[serde(skip_serializing_if = "Option::is_none", rename = "AllowedForSSO")]
    pub allowed_for_sso: Option<bool>,
}

/// Response from the update domain flags endpoint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCorePutDomainsRes {
    /// The updated domain details
    pub domain: LtCoreDomainOutput,
}

impl LtContract for LtCorePutDomainsReq {
    type Response = LtSlimAPIJSON<LtCorePutDomainsRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/core/v4/domains/{}", self.domain_id)))
    }
}

impl AuthReq for LtCorePutDomainsReq {}
