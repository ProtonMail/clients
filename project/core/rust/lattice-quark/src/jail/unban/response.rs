use std::str::FromStr;

use lattice::LatticeError;

/// Any plain-text body is accepted (via [`LtQuarkResTryFrom`]).
#[derive(Debug, PartialEq, Eq)]
pub struct LtQuarkJailUnbanRes;

impl FromStr for LtQuarkJailUnbanRes {
    type Err = LatticeError;

    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        Ok(Self)
    }
}
