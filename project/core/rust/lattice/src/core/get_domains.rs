use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtSerdeQueryParams, LtSlimAPIJSON, LtSlimApiPageQuery,
};

use super::post_domains::LtCoreDomainOutput;

pub const MAX_PAGE_SIZE: u32 = 150;

/// Request to get all domains for the user's organization
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct LtCoreGetDomainsReq {
    pub pagination: LtSlimApiPageQuery<MAX_PAGE_SIZE>,
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

impl LtContract for LtCoreGetDomainsReq {
    type Response = LtSlimAPIJSON<LtCoreGetDomainsRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtSerdeQueryParams<&'q LtSlimApiPageQuery<MAX_PAGE_SIZE>>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/domains"))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        Some(LtSerdeQueryParams(&self.pagination))
    }
}

impl AuthReq for LtCoreGetDomainsReq {}
