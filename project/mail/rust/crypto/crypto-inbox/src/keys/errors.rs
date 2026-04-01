use proton_crypto_account::errors::EncryptionPreferencesError;

#[derive(Debug, thiserror::Error)]
#[allow(clippy::module_name_repetitions)]
pub enum SendPreferencesError {
    #[error("Failed to create encryption preferences for send preferences: {0}")]
    EncryptionPreferences(#[from] EncryptionPreferencesError),
}

#[derive(Debug, thiserror::Error)]
#[allow(clippy::module_name_repetitions)]
pub enum CryptoPackageTypeError {
    #[error("Failed to convert {0} to a package type.")]
    Parse(u8),
}
