use lattice::LatticeError;

use crate::{LtQuarkContract, LtQuarkFormat, LtQuarkJSONRes, QuarkCommand};

use super::LtQuarkUserCreateOrganizationRes;

/// Create a new organization and assign an administrator
/// Equivalent of ./quark user:create:organization [options] [--] <userID> <password>
#[derive(Default)]
pub struct LtQuarkUserCreateOrganization {
    /// Admin's UserID
    pub user_id: u64,
    /// Admin's password
    pub password: String,
    /// Space quota for the administrator, in GB
    pub space: Option<u32>,
    /// VPN quota for the administrator
    pub vpn: Option<u32>,
    /// Organization password
    pub org_password: Option<String>,
    /// Organization salt (base64)
    pub org_salt: Option<String>,
    /// Organization name
    pub org_name: Option<String>,
}

impl LtQuarkContract for LtQuarkUserCreateOrganization {
    const COMMAND_PATH: &'static str = "user:create:organization";
    type Response = LtQuarkJSONRes<LtQuarkUserCreateOrganizationRes>;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        Ok(QuarkCommand::default()
            .query_if_some("-s", self.space)
            .query_if_some("--vpn", self.vpn)
            .query_if_some("-p", self.org_password.as_ref())
            .query_if_some("--orgSalt", self.org_salt.as_ref())
            .query_if_some("--orgName", self.org_name.as_ref())
            .query("-f", LtQuarkFormat::Json)
            .value(self.user_id)
            .value(&self.password))
    }
}
