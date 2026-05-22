use std::str::FromStr;

use lattice::LatticeError;

/// Decimal `u64` on a single line of plain text (via [`LtQuarkResTryFrom`]).
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
