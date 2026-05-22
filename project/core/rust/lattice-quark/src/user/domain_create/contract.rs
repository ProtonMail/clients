use lattice::LatticeError;

use crate::{LtQuarkContract, QuarkCommand};

use super::LtQuarkOrganizationCreateDomainRes;

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
