use std::str::FromStr;

use crate::{
    LatticeError,
    quark::{LtQuarkContract, LtQuarkResTryFrom, QuarkCommand},
};

/// Add an event to the quark system state
/// Equivalent of ./quark event:add --uid <session_id> -- <username> <event_type> <item_id> <event_action>
pub struct LtQuarkEncryptionIdDec {
    pub enc_id: String,
}

#[derive(Debug, Clone, Copy)]
pub struct LtQuarkEncryptionIdDecRes(pub u64);

impl FromStr for LtQuarkEncryptionIdDecRes {
    type Err = LatticeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u64>()
            .map_err(|e| LatticeError::UnexpectedResponse(e.to_string()))
            .map(LtQuarkEncryptionIdDecRes)
    }
}
impl LtQuarkContract for LtQuarkEncryptionIdDec {
    const COMMAND_PATH: &'static str = "encryption:id";
    type Response = LtQuarkResTryFrom<LtQuarkEncryptionIdDecRes>;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        Ok(QuarkCommand::default().query_flag("-d").value(&self.enc_id))
    }
}
