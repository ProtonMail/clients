use lattice::LatticeError;

use super::{LtQuarkRes, QuarkCommand};

pub trait LtQuarkContract {
    /// Response parser for this command's stdout; must match the wire format (JSON vs plain text).
    type Response: LtQuarkRes;

    const COMMAND_PATH: &'static str;

    fn params(&self) -> Result<QuarkCommand, LatticeError>;
}
