use base64::DecodeError;
use ical::parser::ParserError;
use mail_vcard::VCardError;
use proton_crypto_account::{errors::CardCryptoError, proton_crypto::CryptoError};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ContactKeyExtractionError>;

#[derive(Debug, Error)]
pub enum ContactKeyExtractionError {
    #[error("no key data in card to import")]
    NoData,
    #[error("error decoding Base64 data: {0}")]
    Base64Decode(#[from] DecodeError),
    #[error("error importing PGP key: {0}")]
    PGPError(#[from] CryptoError),
    #[error("no vcard found in signed card data")]
    NoVCard,
    #[error("failed to parse vcard: {0}")]
    VCardParse(#[from] ParserError),
    #[error("failed to validate vcard: {0}")]
    VCardValidation(#[from] VCardError),
    #[error("failed to verify vcard: {0}")]
    VCardVerification(#[from] CardCryptoError),
}
