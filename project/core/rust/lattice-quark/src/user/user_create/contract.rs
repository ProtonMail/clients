use lattice::LatticeError;

use crate::{LtQuarkContract, LtQuarkFormat, LtQuarkJSONRes, QuarkCommand};

use super::super::{LtQuarkKeyType, LtQuarkUserStatus};
use super::LtQuarkUserCreateRes;

/// Create a new user in the quark system.
///
/// Quark CLI equivalent:
///
/// ```text
/// ./quark user:create [options]
/// ```
pub struct LtQuarkUserCreate {
    pub name: String,
    pub password: String,
    pub recovery_email: Option<String>,
    pub status: Option<LtQuarkUserStatus>,
    pub auth: Option<u8>,
    // Create the user's default address, will not automatically setup the address key
    pub create_address: bool,
    pub gen_keys: Option<LtQuarkKeyType>,
    pub mailbox_pass: Option<String>,
    pub external: Option<bool>,
    pub external_email: Option<String>,
    pub totp_secret: Option<String>,
    pub temp_password: Option<bool>,
}

impl Default for LtQuarkUserCreate {
    fn default() -> Self {
        Self {
            name: "proton987".to_string(),
            password: "12341234".to_string(),
            recovery_email: None,
            status: None,
            auth: None,
            create_address: false,
            gen_keys: None,
            mailbox_pass: None,
            external: None,
            external_email: None,
            totp_secret: None,
            temp_password: None,
        }
    }
}

impl LtQuarkContract for LtQuarkUserCreate {
    const COMMAND_PATH: &'static str = "user:create";
    type Response = LtQuarkJSONRes<LtQuarkUserCreateRes>;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        Ok(QuarkCommand::default()
            .query("-N", &self.name)
            .query("-p", &self.password)
            .query_if_some("-r", self.recovery_email.as_ref())
            .query_if_some("-s", self.status.as_ref())
            .query_if_some("-k", self.gen_keys.as_ref())
            .query_if_some("-m", self.mailbox_pass.as_ref())
            .query_if_some("-e", self.external.as_ref())
            .query_if_some("--external-email", self.external_email.as_ref())
            .query_if_some("--totp-secret", self.totp_secret.as_ref())
            .query_flag_if(self.temp_password == Some(true), "--temp-password")
            .query("-f", LtQuarkFormat::Json))
    }
}
