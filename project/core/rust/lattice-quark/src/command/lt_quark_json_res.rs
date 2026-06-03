use derive_more::Deref;
use lattice::LatticeError;
use serde::de::DeserializeOwned;

use super::LtQuarkRes;

/// JSON object body (`serde` deserialize). Request the command with `-f json` ([`super::LtQuarkFormat::Json`]).
#[derive(Debug, Clone, Copy, Deref)]
pub struct LtQuarkJSONRes<T: DeserializeOwned>(pub T);

impl<T: DeserializeOwned> LtQuarkRes for LtQuarkJSONRes<T> {
    fn from_quark_body(body: &[u8]) -> Result<Self, LatticeError> {
        let api_response: T = serde_json::from_slice::<T>(body)
            .map_err(|e| LatticeError::SerdeJSON(e, String::from_utf8(body.to_vec()).ok()))?;
        Ok(LtQuarkJSONRes(api_response))
    }
}
