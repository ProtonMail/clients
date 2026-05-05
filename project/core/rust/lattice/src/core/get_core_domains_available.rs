use std::{borrow::Cow, collections::HashMap, iter::once};

use crate::{LatticeError, LtContract, LtRequestQueryParams, LtSlimAPIJSON, Sensitive, UnauthReq};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetDomainsAvailableReq {
    pub domain_type: Option<String>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetDomainsAvailableRes {
    pub domains: Vec<String>,
}

pub struct LtCoreGetDomainsAvailableQueryParams<'a> {
    pub domain_type: &'a str,
}

impl LtRequestQueryParams for LtCoreGetDomainsAvailableQueryParams<'_> {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, crate::Sensitive<String>>, LatticeError> {
        Ok(once(("Type".into(), Sensitive::new(self.domain_type.to_owned()))).collect())
    }
}

impl LtContract for LtCoreGetDomainsAvailableReq {
    type Response = LtSlimAPIJSON<LtCoreGetDomainsAvailableRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtCoreGetDomainsAvailableQueryParams<'q>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/domains/available"))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        self.domain_type
            .as_ref()
            .map(|domain_type| LtCoreGetDomainsAvailableQueryParams {
                domain_type: domain_type.as_str(),
            })
    }
}

impl UnauthReq for LtCoreGetDomainsAvailableReq {}
