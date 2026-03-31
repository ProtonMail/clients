use proton_crypto_account::{
    errors::{AccountCryptoError, EncryptionPreferencesError, KeyError, KeySelectionError},
    proton_crypto::CryptoError,
};
use thiserror::Error;

use crate::{UserId, ids::AddressId};

pub type LoadingResult<T> = std::result::Result<T, LoadingError>;

#[derive(Debug, Error)]
pub enum KeyHandlingError {
    #[error("No issuer found for id {0}")]
    NoUser(UserId),
    #[error("No address found for id {0}")]
    NoAddress(AddressId),
    #[error("No user secret found")]
    NoUserSecret,
    #[error("No user keys could be unlocked: {0:?}")]
    UserKeyUnlock(Vec<KeyError>),
    #[error("No address keys could be unlocked: {0:?}")]
    AddressKeyUnlock(Vec<KeyError>),
    #[cfg(feature = "contacts")]
    #[error("Failed to extract pinned keys from contact card: {0}")]
    VCardKeyExtraction(#[from] mail_crypto_contact_keys::ContactKeyExtractionError),
    #[error(transparent)]
    Loading(#[from] LoadingError),
    #[error("Failed to import public address keys: {0}")]
    PublicKeyImport(#[from] AccountCryptoError),
    #[error("No primary user key found")]
    NoPrimaryKey,
    #[error("No primary address key found")]
    NoPrimaryAddressKey,
    #[error("Failed to select primary address key for mail encryption: {0}")]
    PrimaryAddressKeyForMailEncryption(#[from] KeySelectionError),
    #[error("Failed to create encryption preferences: {0}")]
    EncryptionPreferences(#[from] EncryptionPreferencesError),
    #[error("No public key loader found")]
    NoPublicKeyLoader,
    #[error("Failed to serialize cache key: {0}")]
    CacheKeySerialization(#[from] CryptoError),
    #[error("Failed to build key selector: {0}")]
    Build(#[from] KeyManagerBuilderError),
}

#[derive(Debug, Error)]
pub enum LoadingError {
    #[error("Failed to load keys via API: {0}")]
    Api(#[from] ApiError),
    #[error("Failed to load keys via database: {0}")]
    Database(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Failed to load keys: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Error)]

pub enum ApiError {
    #[error("API error (code {code}): {error:?} | details: {details:?}")]
    Api {
        /// Internal API code.
        code: u32,

        /// Optional error message that may be present.
        error: Option<String>,

        /// Optional JSON type with error details.
        details: Option<String>,
    },
    #[error(transparent)]
    Network(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KeyManagerBuilderError {
    #[error("missing user_id")]
    MissingUserId,
    #[error("missing secret_loader")]
    MissingSecretLoader,
    #[error("missing private_key_loader")]
    MissingPrivateKeyLoader,
}
