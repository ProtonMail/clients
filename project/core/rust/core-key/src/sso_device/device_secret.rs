use std::fmt;

use data_encoding::BASE64;
use lattice::Sensitive;
use proton_crypto::crypto::{
    DataEncoding, Decryptor, DecryptorSync, Encryptor, EncryptorSync, PGPProvider, PGPProviderSync,
    VerifiedData,
};
use proton_crypto_account::keys::APIPublicAddressKeys;
use zeroize::Zeroize;

use crate::SharedCryptoError;
use crate::sso_device::display_code::{DeviceDisplayCode, DeviceDisplayCodeError};

fn import_primary_address_public_key<P: PGPProviderSync>(
    pgp: &P,
    address_keys: &APIPublicAddressKeys,
) -> Result<P::PublicKey, SharedCryptoError> {
    let primary = address_keys
        .address_keys
        .keys
        .iter()
        .find(|k| k.primary)
        .ok_or(SharedCryptoError::NoPrimaryAddressPublicKey)?;
    Ok(pgp.public_key_import(&primary.public_key, DataEncoding::Armor)?)
}

#[derive(Clone, Copy, Zeroize)]
pub struct DeviceSecret(pub [u8; 32]);

impl fmt::Debug for DeviceSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeviceSecret([REDACTED])")
    }
}

impl DeviceSecret {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn try_from_bytes(bytes: &[u8]) -> Option<Self> {
        bytes.try_into().ok().map(Self::from_bytes)
    }

    /// Random 32-byte device secret from the platform CSPRNG.
    pub fn random() -> Self {
        Self::from_bytes(proton_crypto::generate_secure_random_bytes::<32>())
    }

    pub fn display_code(&self) -> DeviceDisplayCode {
        DeviceDisplayCode::from_secret_bytes(&self.0)
    }

    fn from_activation_secret_b64(secret_b64: &str) -> Result<Self, SharedCryptoError> {
        let secret_bytes = BASE64.decode(secret_b64.as_bytes())?;
        if secret_bytes.len() != 32 {
            return Err(SharedCryptoError::InvalidDeviceSecretLength {
                expected: 32,
                actual: secret_bytes.len(),
            });
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&secret_bytes);
        Ok(Self(bytes))
    }

    /// Decrypt activation token, validate confirmation code (parse-first), return device secret.
    pub fn from_activation<P: PGPProviderSync>(
        pgp: &P,
        decrypt_keys: &[&P::PrivateKey],
        activation_token: &str,
        typed_code: &DeviceDisplayCode,
    ) -> Result<Self, SharedCryptoError> {
        let secret_b64 =
            Self::decrypt_activation_token_armored(pgp, decrypt_keys, activation_token)?;
        let device_secret = Self::from_activation_secret_b64(&secret_b64)?;
        if &device_secret.display_code() != typed_code {
            return Err(DeviceDisplayCodeError::Mismatch.into());
        }
        Ok(device_secret)
    }

    fn as_base64(&self) -> Sensitive<String> {
        Sensitive::new(BASE64.encode(&self.0))
    }

    pub fn encrypt_activation_token<P: PGPProviderSync>(
        &self,
        pgp: &P,
        public_key: &<P as PGPProvider>::PublicKey,
    ) -> Result<Sensitive<String>, SharedCryptoError> {
        let device_secret_base64 = self.as_base64();
        let encrypted = pgp
            .new_encryptor()
            .with_encryption_key(public_key)
            .encrypt_raw(device_secret_base64.as_bytes(), DataEncoding::Armor)?;
        Ok(Sensitive::new(String::from_utf8(encrypted)?))
    }

    pub fn encrypt_activation_token_from_address_keys<P: PGPProviderSync>(
        &self,
        pgp: &P,
        address_keys: &APIPublicAddressKeys,
    ) -> Result<Sensitive<String>, SharedCryptoError> {
        let public_key = import_primary_address_public_key(pgp, address_keys)?;
        self.encrypt_activation_token(pgp, &public_key)
    }

    pub fn decrypt_activation_token_armored<P: PGPProviderSync>(
        pgp: &P,
        address_private_keys: &[&P::PrivateKey],
        activation_token: &str,
    ) -> Result<String, SharedCryptoError> {
        let verified = pgp
            .new_decryptor()
            .with_decryption_key_refs(address_private_keys)
            .decrypt(activation_token.as_bytes(), DataEncoding::Armor)?;
        Ok(String::from_utf8(verified.to_vec())?)
    }
}
