use derive_more::Display;

#[cfg(feature = "serde")]
use crate::LtApiResponseError;

/// An error type for Lattice operations.
///
/// This error type is used to wrap errors from the `serde_json` crate.
#[derive(derive_more::Debug, Display)]
pub enum LatticeError {
    #[cfg(feature = "serde")]
    #[display("SerdeJSON: {_0} {_1:?}")]
    SerdeJSON(serde_json::Error, Option<String>),

    #[display("UnexpectedResponse: {_0}")]
    UnexpectedResponse(String),

    #[display("UnexpectedStatusCode({_0}: \"{}\")", String::from_utf8(_1.to_vec()).unwrap_or_else(|_| format!("Invalid UTF-8: {:?}", _1)))]
    #[debug("UnexpectedStatusCode({_0}: \"{}\")", String::from_utf8(_1.to_vec()).unwrap_or_else(|_| format!("Invalid UTF-8: {:?}", _1)))]
    UnexpectedStatusCode(u16, Vec<u8>),

    #[cfg(feature = "serde")]
    #[display("ApiError Status({_0}), {_1:?}")]
    ApiError(u16, Box<LtApiResponseError>),

    #[cfg(feature = "serde")]
    #[display("SerdeQs: {_0}")]
    SerdeQs(serde_qs::Error),

    /// Contract construction, parsing, or other **non-transport** failures that do not fit
    /// the structured variants above. Callers using Muon (`lattice-muon1` / `lattice-muon2`)
    /// should surface HTTP/network errors as those crates' `Error::Transport` (or native
    /// transport errors), not by stuffing them into `Other`.
    #[display("Other: {_0}")]
    Other(String),
}

impl std::error::Error for LatticeError {}

impl LatticeError {
    #[cfg(feature = "serde")]
    pub fn as_api_error(&self) -> Option<&LtApiResponseError> {
        if let Self::ApiError(_, error) = self {
            Some(error)
        } else {
            None
        }
    }
}

#[cfg(feature = "serde")]
impl From<serde_qs::Error> for LatticeError {
    fn from(value: serde_qs::Error) -> Self {
        Self::SerdeQs(value)
    }
}
