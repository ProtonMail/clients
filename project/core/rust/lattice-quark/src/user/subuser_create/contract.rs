use lattice::LatticeError;

use crate::{LtQuarkContract, LtQuarkFormat, LtQuarkJSONRes, QuarkCommand};

use super::super::LtQuarkKeyType;
use super::LtQuarkUserCreateSubuserRes;

/// Create a new subuser attached to an existing organization.
///
/// Quark CLI equivalent:
///
/// ```text
/// ./quark user:create:subuser [options] [--] <ownerUserID> <ownerPassword>
/// ```
#[derive(Default)]
pub struct LtQuarkUserCreateSubuser {
    /// UserID of admin user
    pub owner_user_id: u64,
    /// Account password of admin user
    pub owner_password: String,
    /// New user's name
    pub name: Option<String>,
    /// New user's password
    pub password: Option<String>,
    /// New user's status
    pub status: Option<u8>,
    /// Auth version used for password generation (0 -> 4)
    pub auth: Option<u8>,
    /// Set up the user's default address with keys
    pub gen_keys: Option<LtQuarkKeyType>,
    /// Set up the user in 2 password mode
    pub mailbox_pass: Option<String>,
    /// Attach to the organization as a private sub-user
    pub private: Option<bool>,
    /// Space quota for the subuser, in GB
    pub space: Option<u32>,
    /// VPN quota for the subuser
    pub vpn: Option<u32>,
    /// New user's domain
    pub domain: Option<String>,
}

impl LtQuarkContract for LtQuarkUserCreateSubuser {
    const COMMAND_PATH: &'static str = "user:create:subuser";
    type Response = LtQuarkJSONRes<LtQuarkUserCreateSubuserRes>;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        Ok(QuarkCommand::default()
            .query_if_some("-N", self.name.as_ref())
            .query_if_some("-p", self.password.as_ref())
            .query_if_some("-s", self.status)
            .query_if_some("--auth", self.auth)
            .query_if_some("-k", self.gen_keys)
            .query_if_some("-m", self.mailbox_pass.as_ref())
            .query_flag_if(self.private == Some(true), "--private")
            .query_if_some("--space", self.space)
            .query_if_some("--vpn", self.vpn)
            .query_if_some("-d", self.domain.as_ref())
            .query("-f", LtQuarkFormat::Json)
            .value(self.owner_user_id)
            .value(&self.owner_password))
    }
}
