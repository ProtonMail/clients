use std::string::FromUtf8Error;

use derive_more::{Display, Error, From};
use proton_crypto_subtle::SubtleError;

#[derive(Debug, Display, Error, From)]
pub enum DeviceSecretError {
    #[display("subtle crypto: {_0}")]
    Subtle(#[from] SubtleError),
    #[display("base64 decode: {_0}")]
    Base64Decode(#[from] data_encoding::DecodeError),
    #[display("device secret must be {expected} bytes, got {actual}")]
    InvalidSecretLength { expected: usize, actual: usize },
    #[display("utf8: {_0}")]
    Utf8(#[from] FromUtf8Error),
    #[from(ignore)]
    #[display("pgp: {_0}")]
    Pgp(#[error(ignore)] String),
}
