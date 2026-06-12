use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

use super::{LtCoreDomainId, post_domains::LtCoreDomainOutput};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCorePutDomainFlagsReq {
    #[serde(skip)]
    pub domain_id: LtCoreDomainId,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_for_mail: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "AllowedForSSO")]
    pub allowed_for_sso: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCorePutDomainFlagsRes {
    pub domain: LtCoreDomainOutput,
}

impl LtContract for LtCorePutDomainFlagsReq {
    type Response = LtSlimAPIJSON<LtCorePutDomainFlagsRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/domains/{}/flags",
            self.domain_id
        )))
    }
}

impl AuthReq for LtCorePutDomainFlagsReq {}
