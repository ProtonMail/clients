//! AES-GCM + gzip helpers shared by sequential batch API and chunked real bodies.

use super::FixtureError;

/// Decrypt a single body using AES-256-GCM
pub(crate) fn decrypt_body(
    encrypted_base64: &str,
    encryption_key_hex: &str,
) -> Result<String, FixtureError> {
    use aes_gcm::{
        Aes256Gcm, Nonce,
        aead::{Aead, KeyInit},
    };
    use base64::{Engine, engine::general_purpose::STANDARD};

    // Decode key from hex
    let key_bytes = hex::decode(encryption_key_hex)
        .map_err(|e| FixtureError::DecryptionError(e.to_string()))?;

    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| FixtureError::DecryptionError(e.to_string()))?;

    // Decode body from base64
    let encrypted = STANDARD
        .decode(encrypted_base64)
        .map_err(|e| FixtureError::DecryptionError(format!("Base64 decode error: {e}")))?;

    if encrypted.len() < 12 {
        return Err(FixtureError::DecryptionError(
            "Encrypted data too short".to_string(),
        ));
    }

    // Split nonce (first 12 bytes) and ciphertext
    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| FixtureError::DecryptionError(format!("Decryption failed: {e}")))?;

    // Decompress (gzip) — bodies are compressed before encryption on upload
    let decompressed = decompress_gzip(&plaintext)?;

    String::from_utf8(decompressed)
        .map_err(|e| FixtureError::DecryptionError(format!("UTF-8 decode error: {e}")))
}

/// Decompress gzip-compressed data. Returns the original bytes if not gzip.
pub(crate) fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, FixtureError> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    // Check for gzip magic bytes (0x1f, 0x8b)
    if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| FixtureError::DecryptionError(format!("Gzip decompress error: {e}")))?;
        Ok(decompressed)
    } else {
        // Not gzip — return as-is (backward compatibility with uncompressed bodies)
        Ok(data.to_vec())
    }
}
