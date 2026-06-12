use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashMap};

use crate::{AuthReq, LatticeError, LtContract, LtRequestQueryParams, LtSlimAPIJSON, Sensitive};

use super::{LtCoreDomainId, post_domains::LtCoreDomainOutput};

/// Request to get a specific domain and its DNS check
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LtCoreGetDomainReq {
    /// The domain ID (enc_id in path)
    #[serde(skip)]
    pub domain_id: LtCoreDomainId,

    /// When `Some`, requests a DNS verification refresh (`true` → query parameter `Refresh=1`, `false` → `Refresh=0`).
    /// When `None`, the parameter is omitted and the API uses its default behaviour.
    pub refresh: Option<bool>,
}

/// Response from the get domain endpoint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCoreGetDomainRes {
    /// The domain details
    pub domain: LtCoreDomainOutput,
}

pub struct LtCoreGetDomainQueryParams {
    pub refresh: bool,
}

impl LtRequestQueryParams for LtCoreGetDomainQueryParams {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, crate::Sensitive<String>>, LatticeError> {
        Ok(HashMap::from([(
            "Refresh".into(),
            Sensitive::new(String::from(if self.refresh { "1" } else { "0" })),
        )]))
    }
}

impl LtContract for LtCoreGetDomainReq {
    type Response = LtSlimAPIJSON<LtCoreGetDomainRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtCoreGetDomainQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/core/v4/domains/{}", self.domain_id)))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        self.refresh
            .map(|refresh| LtCoreGetDomainQueryParams { refresh })
    }
}

impl AuthReq for LtCoreGetDomainReq {}
