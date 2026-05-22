use lattice::LatticeError;

use super::LtQuarkRes;

/// Raw UTF-8 body returned unchanged (no trimming or structured parsing).
pub struct LtQuarkResString(pub String);

impl LtQuarkRes for LtQuarkResString {
    fn from_quark_body(body: &[u8]) -> Result<Self, LatticeError> {
        let api_response: String = String::from_utf8(body.to_vec())
            .map_err(|e| LatticeError::UnexpectedResponse(e.to_string()))?;
        Ok(LtQuarkResString(api_response))
    }
}
