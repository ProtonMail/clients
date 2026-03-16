use std::{borrow::Cow, collections::HashMap, iter::once};

use crate::{LatticeError, LtContract, UnauthReq};

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

impl LtContract for LtCoreGetDomainsAvailableReq {
    type Response = LtCoreGetDomainsAvailableRes;
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/domains/available"))
    }

    fn query(&self) -> Result<Option<HashMap<String, String>>, LatticeError> {
        Ok(self
            .domain_type
            .clone()
            .map(|domain_type| once((String::from("Type"), domain_type)).collect()))
    }
}

impl UnauthReq for LtCoreGetDomainsAvailableReq {}
