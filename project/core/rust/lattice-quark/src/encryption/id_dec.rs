use lattice::LatticeError;

use crate::{LtQuarkContract, LtQuarkResTryFrom, QuarkCommand};

use super::LtQuarkEncryptionIdDecRes;

/// Decode an encryption ID in the quark system
/// Equivalent of ./quark encryption:id -d <enc_id>
pub struct LtQuarkEncryptionIdDec {
    pub enc_id: String,
}

impl LtQuarkContract for LtQuarkEncryptionIdDec {
    const COMMAND_PATH: &'static str = "encryption:id";
    type Response = LtQuarkResTryFrom<LtQuarkEncryptionIdDecRes>;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        Ok(QuarkCommand::default().query_flag("-d").value(&self.enc_id))
    }
}
