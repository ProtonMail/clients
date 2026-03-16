use crate::{
    LatticeError,
    quark::{
        LtQuarkContract, LtQuarkFormat, LtQuarkJSONRes, QuarkCommand,
        user::{LtQuarkKeyType, LtQuarkUserStatus},
    },
};

/// Create a new user in the quark system
/// Equivalent of ./quark user:create [options]
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
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct LtQuarkUserCreateRes {
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: String,
    pub name: String,
    pub password: String,
    pub status: LtQuarkUserStatus,
    pub recovery: String,
    pub recovery_verified: u8,
    pub recovery_phone: String,
    pub auth_version: u8,
    #[cfg_attr(feature = "serde", serde(rename = "Created at"))]
    pub created_at: String,
    #[cfg_attr(feature = "serde", serde(rename = "Dec_ID"))]
    pub dec_id: u64,
    pub status_info: String,
    pub mailbox_password: Option<String>,
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
            .query("-f", LtQuarkFormat::Json))
    }
}
