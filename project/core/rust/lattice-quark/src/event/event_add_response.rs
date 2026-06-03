use std::str::FromStr;

use lattice::LatticeError;

/// Exact line `Event added successfully` (via [`crate::LtQuarkResTryFrom`]).
#[derive(Debug, PartialEq, Eq)]
pub enum LtQuarkEventAddResponse {
    Success,
}

impl FromStr for LtQuarkEventAddResponse {
    type Err = LatticeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s == "Event added successfully" {
            return Ok(LtQuarkEventAddResponse::Success);
        }
        Err(LatticeError::UnexpectedResponse(s.to_string()))
    }
}
