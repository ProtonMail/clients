use std::str::FromStr;

use derive_more::Deref;
use lattice::LatticeError;

use super::LtQuarkRes;

/// Plain-text body: UTF-8 stdout with an optional trailing newline removed, then [`FromStr`].
///
/// Example lines: `"42"` (encryption id decode), `"Event added successfully"` (event add).
#[derive(Deref)]
pub struct LtQuarkResTryFrom<T: FromStr<Err = LatticeError>>(pub T);

impl<T: FromStr<Err = LatticeError>> LtQuarkRes for LtQuarkResTryFrom<T> {
    fn from_quark_body(body: &[u8]) -> Result<Self, LatticeError> {
        let body_string: String = String::from_utf8(body.to_vec())
            .map_err(|e| LatticeError::UnexpectedResponse(e.to_string()))?;
        // Remove the trailing newline
        let body_str = body_string.trim_end_matches('\n');
        let api_response: T = T::from_str(body_str)?;
        Ok(LtQuarkResTryFrom(api_response))
    }
}
