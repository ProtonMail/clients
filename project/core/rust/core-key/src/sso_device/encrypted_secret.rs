use data_encoding::BASE64;
use lattice::Sensitive;
use proton_crypto_account::salts::KeySecret;
use proton_crypto_subtle::aead::{AesGcmCiphertext, AesGcmKey};

use crate::SharedCryptoError;

/// Context for AES-GCM encryption of wire `EncryptedSecret`.
pub const ENCRYPTED_SECRET_CONTEXT: &str = "account.device-secret";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EncryptedSecret(pub String);

impl EncryptedSecret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_sensitive(self) -> Sensitive<String> {
        Sensitive::new(self.0)
    }

    pub fn from_key_secret(
        passphrase: &KeySecret,
        device_secret: &[u8],
    ) -> Result<Self, SharedCryptoError> {
        let key = AesGcmKey::from_bytes(device_secret)?;
        let ciphertext = key.encrypt(passphrase.as_ref(), Some(ENCRYPTED_SECRET_CONTEXT))?;
        Ok(Self::new(BASE64.encode(&ciphertext.encode())))
    }

    pub fn decrypt_to_vec(&self, device_secret: &[u8]) -> Result<Vec<u8>, SharedCryptoError> {
        let ciphertext = BASE64.decode(self.as_str().as_bytes())?;
        let key = AesGcmKey::from_bytes(device_secret)?;
        let cipher = AesGcmCiphertext::decode(ciphertext.as_slice())?;
        Ok(key.decrypt(cipher, Some(ENCRYPTED_SECRET_CONTEXT))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device_secret() -> [u8; 32] {
        [0x42; 32]
    }

    #[test]
    fn encrypted_secret_produces_base64_ciphertext_bound_to_context() {
        let passphrase = b"my-mailbox-passphrase";
        let ds = device_secret();
        let encrypted = EncryptedSecret::from_key_secret(&KeySecret::new(passphrase.to_vec()), &ds)
            .expect("encrypt");

        let bytes = BASE64
            .decode(encrypted.as_str().as_bytes())
            .expect("valid base64");
        let key = AesGcmKey::from_bytes(ds).expect("valid aes key");
        let cipher = AesGcmCiphertext::decode(&bytes).expect("valid ciphertext");
        let plaintext = key
            .decrypt(cipher, Some(ENCRYPTED_SECRET_CONTEXT))
            .expect("decrypts with matching context");
        assert_eq!(plaintext, passphrase);

        let round_trip = encrypted.decrypt_to_vec(&ds).expect("decrypt");
        assert_eq!(round_trip, passphrase);
    }

    #[test]
    fn encrypted_secret_is_non_deterministic() {
        let passphrase = KeySecret::new(b"same-input".to_vec());
        let ds = device_secret();
        let a = EncryptedSecret::from_key_secret(&passphrase, &ds).unwrap();
        let b = EncryptedSecret::from_key_secret(&passphrase, &ds).unwrap();
        assert_ne!(a, b, "AES-GCM must use a fresh IV");
    }

    #[test]
    fn encrypted_secret_decrypt_rejects_bad_wire_format() {
        let ds = device_secret();
        let base64_err = EncryptedSecret::new("!!!not-base64!!!")
            .decrypt_to_vec(&ds)
            .unwrap_err();
        assert!(matches!(base64_err, SharedCryptoError::Base64(_)));

        let key = AesGcmKey::from_bytes(ds).unwrap();
        let ciphertext = key.encrypt(b"payload", Some("some.other.context")).unwrap();
        let encoded = BASE64.encode(&ciphertext.encode());
        assert!(EncryptedSecret::new(encoded).decrypt_to_vec(&ds).is_err());
    }

    #[test]
    fn encrypted_secret_invalid_device_secret_length_fails() {
        let err =
            EncryptedSecret::from_key_secret(&KeySecret::new(b"passphrase".to_vec()), &[0u8; 16])
                .unwrap_err();
        assert!(matches!(err, SharedCryptoError::AesGcm(_)));
    }
}
