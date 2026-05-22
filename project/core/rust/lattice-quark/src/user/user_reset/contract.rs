use lattice::LatticeError;

use crate::{LtQuarkContract, LtQuarkFormat, LtQuarkJSONRes, QuarkCommand};

use super::super::LtQuarkKeyType;
use super::LtQuarkUserResetRes;

/// Reset an existing user, deactivating their keys and setting a new password.
/// Equivalent of ./quark user:reset [options] -- <userID> <newPassword>
pub struct LtQuarkUserReset {
    pub user_id: u64,
    pub password: String,
    pub gen_keys: Option<LtQuarkKeyType>,
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
