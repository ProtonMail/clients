//! Passphrase provider abstraction.

use async_trait::async_trait;
use secrecy::SecretSlice;

/// Error returned when acquiring a passphrase fails.
#[derive(Debug, thiserror::Error)]
pub enum PassphraseAcquireError {
    #[error("Could not find logged in primary account")]
    NoPrimaryAccount,

    #[error("Could not find session id")]
    NoSessionId,

    #[error("No key_secret for the session")]
    KeySecretDecryption,

    #[error("Could not find session")]
    NoSession,

    #[error("Context error: {0}")]
    Other(#[from] anyhow::Error),
}

/// A trait for acquiring the session passphrase.
#[async_trait]
pub trait PassphraseProvider: Send + Sync {
    async fn get_session_passphrase(&self) -> Result<SecretSlice<u8>, PassphraseAcquireError>;
}
