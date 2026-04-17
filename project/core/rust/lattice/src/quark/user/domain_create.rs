use std::sync::LazyLock;

use regex::Regex;

use crate::{
    LatticeError,
    quark::{LtQuarkContract, LtQuarkRes, QuarkCommand},
};

static DOMAIN_CREATED_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Domain created with ID: (\d+)").expect("static regex"));
static CREATING_DOMAIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Creating domain: ([^ ]+) for Organization with ID: (\d+)").expect("static regex")
});

/// Create a new domain for a given organization
/// Equivalent of ./quark organization:create:domain [options] [--] <organization-id>
#[derive(Default)]
pub struct LtQuarkOrganizationCreateDomain {
    /// Organization ID
    pub organization_id: u64,
    /// The domain name of the domain to be created
    pub domain_name: Option<String>,
    /// The domain flags
    pub flags: Option<u32>,
    /// The domain state
    pub state: Option<u32>,
}

/*
Create Domain
=============
Creating domain: dummydomain95308.com for Organization with ID: 3071
Domain created with ID: 1639
 */
#[derive(Debug, Clone)]
pub struct LtQuarkOrganizationCreateDomainRes {
    pub domain_id: u64,
    pub domain_name: String,
    pub organization_id: u64,
}

impl LtQuarkRes for LtQuarkOrganizationCreateDomainRes {
    fn from_muon_res(response: &muon::http::HttpRes) -> Result<Self, LatticeError> {
        let body = response.body();
        let body_str = String::from_utf8_lossy(body);

        let captures1 = DOMAIN_CREATED_ID_RE
            .captures(&body_str)
            .ok_or_else(|| LatticeError::UnexpectedResponse(body_str.to_string()))?;
        let captures2 = CREATING_DOMAIN_RE
            .captures(&body_str)
            .ok_or_else(|| LatticeError::UnexpectedResponse(body_str.to_string()))?;

        let domain_id = captures1
            .get(1)
            .ok_or_else(|| {
                LatticeError::UnexpectedResponse(
                    "organization:create:domain: missing domain id capture".to_string(),
                )
            })?
            .as_str()
            .parse::<u64>()
            .map_err(|e| {
                LatticeError::UnexpectedResponse(format!(
                    "organization:create:domain: invalid domain id: {e}"
                ))
            })?;
        let domain_name = captures2
            .get(1)
            .ok_or_else(|| {
                LatticeError::UnexpectedResponse(
                    "organization:create:domain: missing domain name capture".to_string(),
                )
            })?
            .as_str()
            .to_string();
        let organization_id = captures2
            .get(2)
            .ok_or_else(|| {
                LatticeError::UnexpectedResponse(
                    "organization:create:domain: missing organization id capture".to_string(),
                )
            })?
            .as_str()
            .parse::<u64>()
            .map_err(|e| {
                LatticeError::UnexpectedResponse(format!(
                    "organization:create:domain: invalid organization id: {e}"
                ))
            })?;

        Ok(LtQuarkOrganizationCreateDomainRes {
            domain_id,
            domain_name,
            organization_id,
        })
    }
}

impl LtQuarkContract for LtQuarkOrganizationCreateDomain {
    const COMMAND_PATH: &'static str = "organization:create:domain";
    type Response = LtQuarkOrganizationCreateDomainRes;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        Ok(QuarkCommand::default()
            .query_if_some("--domain-name", self.domain_name.as_ref())
            .query_if_some("--flags", self.flags)
            .query_if_some("--state", self.state)
            .value(self.organization_id))
    }
}
