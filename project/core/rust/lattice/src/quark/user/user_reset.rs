use crate::{
    LatticeError,
    quark::{LtQuarkContract, LtQuarkFormat, LtQuarkJSONRes, QuarkCommand, user::LtQuarkKeyType},
};

/// Reset an existing user, deactivating their keys and setting a new password.
/// Equivalent of ./quark user:reset [options] -- <userID> <newPassword>
pub struct LtQuarkUserReset {
    pub user_id: u64,
    pub password: String,
    pub gen_keys: Option<LtQuarkKeyType>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct LtQuarkUserResetRes {
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: String,
    pub name: String,
    pub password: String,
    #[cfg_attr(feature = "serde", serde(rename = "Dec_ID"))]
    pub dec_id: u64,
}

impl LtQuarkContract for LtQuarkUserReset {
    const COMMAND_PATH: &'static str = "user:reset";
    type Response = LtQuarkJSONRes<LtQuarkUserResetRes>;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        Ok(QuarkCommand::default()
            .query_if_some("-k", self.gen_keys.as_ref())
            .query("-f", LtQuarkFormat::Json)
            .value(self.user_id)
            .value(&self.password))
    }
}
