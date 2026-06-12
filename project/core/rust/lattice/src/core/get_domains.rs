use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::num::NonZeroU32;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtPaginable, LtSlimAPIJSON};

use super::post_domains::LtCoreDomainOutput;

/// Request to get all domains for the user's organization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct LtCoreGetDomainsReq;

/// Response from the get domains endpoint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCoreGetDomainsRes {
    /// Array of domain objects
    pub domains: Vec<LtCoreDomainOutput>,

    /// Total number of domains
    pub total: u32,
}

impl LtContract for LtCoreGetDomainsReq {
    type Response = LtSlimAPIJSON<LtCoreGetDomainsRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/domains"))
    }
}

impl LtPaginable for LtCoreGetDomainsReq {
    type Item = LtCoreDomainOutput;
    const MAX_PAGE_SIZE: NonZeroU32 = NonZeroU32::new(150).unwrap();

    fn page_items(
        res: LtSlimAPIJSON<LtCoreGetDomainsRes>,
    ) -> (Option<u32>, Vec<LtCoreDomainOutput>) {
        (Some(res.0.total), res.0.domains)
    }
}

impl AuthReq for LtCoreGetDomainsReq {}
