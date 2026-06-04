use data_encoding::BASE64;
use proton_crypto::crypto::{
    DataEncoding, Decryptor, DecryptorSync, Encryptor, EncryptorSync, PGPProviderSync, VerifiedData,
};
use proton_crypto_subtle::aead::{AesGcmCiphertext, AesGcmKey};
use sha2::{Digest, Sha256};

use super::device_secret_error::DeviceSecretError;

const ENCRYPTED_SECRET_CONTEXT: &str = "account.device-secret";

#[derive(Clone, Copy, Debug)]
pub struct DeviceSecret(pub [u8; 32]);

impl DeviceSecret {
    pub fn random() -> Self {
        let mut secret = [0u8; 32];
        rand::Rng::fill(&mut rand::rng(), &mut secret);
        Self(secret)
    }

    pub fn display_code(&self) -> String {
        let secret_base64 = BASE64.encode(&self.0);
        let hash = Sha256::digest(secret_base64.as_bytes());
        let hash_string = format!("{hash:x}");
        let encoded = crockford_base32_encode_bytes(hash_string.as_bytes());
        encoded.chars().take(4).collect()
    }

    pub fn encrypt_activation_token<P: PGPProviderSync>(
        &self,
        pgp: &P,
        public_key_armor: &str,
    ) -> Result<String, DeviceSecretError> {
        let public_key = pgp
            .public_key_import(public_key_armor.as_bytes(), DataEncoding::Armor)
            .map_err(|e| DeviceSecretError::Pgp(e.to_string()))?;
        let device_secret_base64 = BASE64.encode(&self.0);
        let encrypted = pgp
            .new_encryptor()
            .with_encryption_key(&public_key)
            .encrypt_raw(device_secret_base64.as_bytes(), DataEncoding::Armor)
            .map_err(|e| DeviceSecretError::Pgp(e.to_string()))?;
        String::from_utf8(encrypted).map_err(DeviceSecretError::Utf8)
    }

    pub fn decrypt_activation_token_armored<P: PGPProviderSync>(
        pgp: &P,
        address_private_keys: &[&P::PrivateKey],
        activation_token: &str,
    ) -> Result<String, DeviceSecretError> {
        let verified = pgp
            .new_decryptor()
            .with_decryption_key_refs(address_private_keys)
            .decrypt(activation_token.as_bytes(), DataEncoding::Armor)
            .map_err(|e| DeviceSecretError::Pgp(e.to_string()))?;
        String::from_utf8(verified.to_vec()).map_err(DeviceSecretError::Utf8)
    }

    pub fn encrypt_passphrase(&self, passphrase_utf8: &[u8]) -> Result<String, DeviceSecretError> {
        let key = AesGcmKey::from_bytes(self.0).map_err(DeviceSecretError::Subtle)?;
        let ciphertext = key
            .encrypt(passphrase_utf8, Some(ENCRYPTED_SECRET_CONTEXT))
            .map_err(DeviceSecretError::Subtle)?;
        Ok(BASE64.encode(&ciphertext.encode()))
    }

    pub fn decrypt_encrypted_secret(
        &self,
        encrypted_secret_b64: &str,
    ) -> Result<Vec<u8>, DeviceSecretError> {
        let ciphertext = BASE64
            .decode(encrypted_secret_b64.as_bytes())
            .map_err(DeviceSecretError::Base64Decode)?;
        let key = AesGcmKey::from_bytes(self.0).map_err(DeviceSecretError::Subtle)?;
        let cipher =
            AesGcmCiphertext::decode(ciphertext.as_slice()).map_err(DeviceSecretError::Subtle)?;
        key.decrypt(cipher, Some(ENCRYPTED_SECRET_CONTEXT))
            .map_err(DeviceSecretError::Subtle)
    }
}

fn crockford_base32_encoding() -> data_encoding::Encoding {
    let mut spec = data_encoding::Specification::new();
    spec.symbols.push_str("0123456789ABCDEFGHJKMNPQRSTVWXYZ");
    spec.padding = None;
    spec.encoding()
        .expect("Crockford base32 spec should be valid")
}

fn crockford_base32_encode_bytes(bytes: &[u8]) -> String {
    crockford_base32_encoding().encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_code_known_vector() {
        let secret = DeviceSecret([0u8; 32]);
        assert_eq!(
            BASE64.encode(&secret.0),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
        );
        let code = secret.display_code();
        assert_eq!(code.len(), 4);
        assert_eq!(code, "6MRK");
    }
}
