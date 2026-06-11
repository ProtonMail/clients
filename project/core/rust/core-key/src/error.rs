use proton_crypto::crypto::VerificationError;
use proton_crypto_account::errors::{AccountCryptoError, KeyError, SKLError};
use proton_crypto_account::keys::KeyId;
use proton_crypto_account::proton_crypto::CryptoError;
use proton_crypto_account::salts::SaltError;
use thiserror::Error;

use crate::DeviceDisplayCodeError;

#[derive(Debug, Error)]
pub enum SharedCryptoError {
    #[error("salt: {0}")]
    Salt(#[from] SaltError),

    #[error("aes-gcm: {0}")]
    AesGcm(#[from] proton_crypto_subtle::SubtleError),

    #[error("verification: {0}")]
    Verification(#[from] VerificationError),

    #[error("crypto: {0}")]
    Crypto(#[from] CryptoError),

    #[error("account crypto: {0}")]
    AccountCrypto(#[from] AccountCryptoError),

    #[error("skl: {0}")]
    SKL(#[from] SKLError),

    #[error("utf8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("no primary user key")]
    NoPrimaryUserKey,

    #[error("no primary public key in address keys")]
    NoPrimaryAddressPublicKey,

    #[error("unlocked key {key_id} not found")]
    UnlockedKeyNotFound { key_id: KeyId },

    #[error("no member user keys unlocked")]
    NoMemberUserKeysUnlocked,

    #[error("address keys unlock failed: {failed:?}")]
    AddressKeysUnlockFailed { failed: Vec<KeyError> },

    #[error("missing org token for key {key_id}")]
    MissingOrgToken { key_id: String },

    #[error("no decrypt keys for activation address {activation_address_id}")]
    NoDecryptKeysForActivation { activation_address_id: String },

    #[error("missing private keys for {member_label}")]
    MissingPrivateKeys { member_label: String },

    #[error("missing activation token for {member_label}")]
    MissingActivationToken { member_label: String },

    #[error("pending auth device missing {field}")]
    PendingAuthDeviceMissingField { field: &'static str },

    #[error("address key {id} missing token")]
    MissingToken { id: String },

    #[error("address key {id} missing signature")]
    MissingSignature { id: String },

    #[error("display code: {0}")]
    DisplayCode(#[from] DeviceDisplayCodeError),

    #[error("invalid device secret length: expected {expected}, actual {actual}")]
    InvalidDeviceSecretLength { expected: usize, actual: usize },

    #[error("base64: {0}")]
    Base64(#[from] data_encoding::DecodeError),
}
