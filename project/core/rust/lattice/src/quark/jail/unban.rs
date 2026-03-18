use std::str::FromStr;

use crate::{
    LatticeError,
    quark::{LtQuarkContract, LtQuarkResTryFrom, QuarkCommand},
};

/// Unban a user from the quark system state
pub struct LtQuarkJailUnban;

impl LtQuarkContract for LtQuarkJailUnban {
    const COMMAND_PATH: &'static str = "jail:unban";
    type Response = LtQuarkResTryFrom<LtQuarkJailUnbanResponse>;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        Ok(QuarkCommand::default())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct LtQuarkJailUnbanResponse;

impl FromStr for LtQuarkJailUnbanResponse {
    type Err = LatticeError;

    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        Ok(Self)
    }
}
