use base64::prelude::*;
use proton_crypto_subtle::SubtleError;
use proton_crypto_subtle::aead::{AesGcmCiphertext, AesGcmKey};
use serde::Deserialize;
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum ForkPayloadError {
    #[error("invalid key: {0}")]
    InvalidKey(#[source] SubtleError),

    #[error("decrypt: {0}")]
    Decrypt(#[source] SubtleError),

    #[error("decrypt legacy: {0}")]
    DecryptLegacy(#[source] SubtleError),

    #[error("base64 decode: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForkPayload {
    key_password: String,
}

pub fn decode_fork_payload_base64(payload: &str, key: &[u8]) -> Result<Vec<u8>, ForkPayloadError> {
    decode_fork_payload(&BASE64_STANDARD.decode(payload)?, key)
}

pub fn decode_fork_payload(payload: &[u8], key: &[u8]) -> Result<Vec<u8>, ForkPayloadError> {
    let key = AesGcmKey::from_bytes(key).map_err(ForkPayloadError::InvalidKey)?;

    let payload_bytes = decode_decrypt_legacy(payload, &key)
        .inspect_err(|_| debug!("legacy decryption failed, trying non-legacy"))
        .or_else(|_| decode_decrypt(payload, key))?;

    let password = serde_json::from_slice::<ForkPayload>(&payload_bytes)
        .map(|res| res.key_password)
        .map(|res| res.into_bytes())?;

    Ok(password)
}

fn decode_decrypt(payload: &[u8], key: AesGcmKey) -> Result<Vec<u8>, ForkPayloadError> {
    AesGcmCiphertext::decode(payload)
        .and_then(|ciphertext| key.decrypt(ciphertext, None))
        .map_err(ForkPayloadError::Decrypt)
}

fn decode_decrypt_legacy(payload: &[u8], key: &AesGcmKey) -> Result<Vec<u8>, ForkPayloadError> {
    AesGcmCiphertext::decode_legacy(payload)
        .and_then(|ciphertext| key.decrypt_legacy(ciphertext, None))
        .map_err(ForkPayloadError::DecryptLegacy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use serde_json::json;

    const KEY: &[u8; 32] = &[0x42; 32];
    const WRONG_KEY: &[u8; 32] = &[0x13; 32];

    fn encrypt(passphrase: &str) -> Result<Vec<u8>> {
        let key = AesGcmKey::from_bytes(KEY)?;
        let json = json!({ "keyPassword": passphrase });
        let bytes = json.to_string().into_bytes();
        Ok(key.encrypt(bytes, None)?.encode())
    }

    fn encrypt_legacy(passphrase: &str) -> Result<Vec<u8>> {
        let key = AesGcmKey::from_bytes(KEY)?;
        let json = json!({ "keyPassword": passphrase });
        let bytes = json.to_string().into_bytes();
        Ok(key.encrypt_legacy(bytes, None)?.encode())
    }

    #[test]
    fn test_roundtrip() -> Result<()> {
        let passphrase = "another-secret-passphrase";
        let decoded = decode_fork_payload(&encrypt(passphrase)?, KEY)?;
        assert_eq!(decoded, passphrase.as_bytes());
        Ok(())
    }

    #[test]
    fn test_roundtrip_legacy() -> Result<()> {
        let passphrase = "my-super-secret-passphrase";
        let decoded = decode_fork_payload(&encrypt_legacy(passphrase)?, KEY)?;
        assert_eq!(decoded, passphrase.as_bytes());
        Ok(())
    }

    #[test]
    fn test_wrong_key() -> Result<()> {
        let res = decode_fork_payload(&encrypt_legacy("whatever")?, WRONG_KEY);
        assert!(res.is_err());
        Ok(())
    }
}
