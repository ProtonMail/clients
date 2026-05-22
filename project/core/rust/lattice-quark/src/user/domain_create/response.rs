use std::sync::LazyLock;

use regex::Regex;

use lattice::LatticeError;

use crate::LtQuarkRes;

static DOMAIN_CREATED_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Domain created with ID: (\d+)").expect("static regex"));
static CREATING_DOMAIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Creating domain: ([^ ]+) for Organization with ID: (\d+)").expect("static regex")
});

#[derive(Debug, Clone)]
pub struct LtQuarkOrganizationCreateDomainRes {
    pub domain_id: u64,
    pub domain_name: String,
    pub organization_id: u64,
}

impl LtQuarkRes for LtQuarkOrganizationCreateDomainRes {
    fn from_quark_body(body: &[u8]) -> Result<Self, LatticeError> {
        let body_str: String = String::from_utf8(body.to_vec())
            .map_err(|e| LatticeError::UnexpectedResponse(e.to_string()))?;

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
