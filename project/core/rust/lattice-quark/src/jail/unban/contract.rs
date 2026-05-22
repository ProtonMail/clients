use lattice::LatticeError;

use crate::{LtQuarkContract, LtQuarkResTryFrom, QuarkCommand};

use super::LtQuarkJailUnbanRes;

/// Unban a user from the quark system state
pub struct LtQuarkJailUnban;

impl LtQuarkContract for LtQuarkJailUnban {
    const COMMAND_PATH: &'static str = "jail:unban";
    type Response = LtQuarkResTryFrom<LtQuarkJailUnbanRes>;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        Ok(QuarkCommand::default())
    }
}
