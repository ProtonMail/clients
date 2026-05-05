use std::num::NonZeroU32;
use std::{borrow::Cow, collections::HashMap};

use crate::{AuthReq, LatticeError, LtContract, LtRequestQueryParams, LtSlimAPIJSON, Sensitive};

use super::post_domains::LtCoreDomainOutput;

/// Request to get all domains for the user's organization
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct LtCoreGetDomainsReq {
    /// Page size between **1 and 150** (inclusive). Omit to use the API default (typically 150).
    pub page_size: Option<NonZeroU32>,

    /// Zero-based page index. Omit to use the default first page.
    pub page: Option<u32>,
}

/// Response from the get domains endpoint
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCoreGetDomainsRes {
    /// Array of domain objects
    pub domains: Vec<LtCoreDomainOutput>,

    /// Total number of domains
    pub total: u32,
}

pub struct LtCoreGetDomainsQueryParams {
    pub page_size: Option<NonZeroU32>,
    pub page: Option<u32>,
}

impl LtRequestQueryParams for LtCoreGetDomainsQueryParams {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        let mut params = HashMap::new();

        if let Some(page_size) = self.page_size {
            params.insert(
                "PageSize".into(),
                Sensitive::new(page_size.get().to_string()),
            );
        }

        if let Some(page) = self.page {
            params.insert("Page".into(), Sensitive::new(page.to_string()));
        }

        Ok(params)
    }
}

impl LtContract for LtCoreGetDomainsReq {
    type Response = LtSlimAPIJSON<LtCoreGetDomainsRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtCoreGetDomainsQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/domains"))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        if self.page_size.is_some() || self.page.is_some() {
            Some(LtCoreGetDomainsQueryParams {
                page_size: self.page_size,
                page: self.page,
            })
        } else {
            None
        }
    }
}

impl AuthReq for LtCoreGetDomainsReq {}
