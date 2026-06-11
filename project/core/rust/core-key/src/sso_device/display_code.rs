//! Device confirmation display codes (4-character Crockford base32).
//!
//! Legacy derivation (do not change without a compatibility review):
//! 1. Base64-encode the 32-byte device secret
//! 2. SHA-256 hash those base64 *ASCII* bytes
//! 3. Hex-encode the digest as a lowercase string
//! 4. Crockford-base32-encode the hex string's ASCII bytes and take the first 4 symbols

use std::fmt;

use data_encoding::{BASE64, Specification};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DeviceDisplayCodeError {
    #[error("confirmation code must be {expected} characters, got {actual}")]
    WrongLength { expected: usize, actual: usize },

    #[error("invalid character in confirmation code: {character}")]
    InvalidCharacter { character: char },

    #[error("confirmation code contains non-ASCII characters")]
    NotAscii,

    #[error("confirmation code does not match device")]
    Mismatch,
}

/// Four-character Crockford base32 device confirmation code shown to the user.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DeviceDisplayCode([u8; DeviceDisplayCode::LEN]);

impl DeviceDisplayCode {
    pub const LEN: usize = 4;

    /// Parse user input: trim whitespace, uppercase, validate alphabet and length.
    pub fn parse(input: &str) -> Result<Self, DeviceDisplayCodeError> {
        let normalized = input.trim().to_uppercase();
        if !normalized.is_ascii() {
            return Err(DeviceDisplayCodeError::NotAscii);
        }
        let bytes: &[u8] = normalized.as_bytes();
        let Ok(code_array) = bytes.try_into() else {
            return Err(DeviceDisplayCodeError::WrongLength {
                expected: Self::LEN,
                actual: bytes.len(),
            });
        };
        for b in bytes {
            if !is_crockford_char(*b) {
                return Err(DeviceDisplayCodeError::InvalidCharacter {
                    character: *b as char,
                });
            }
        }
        Ok(Self(code_array))
    }

    pub fn from_secret_bytes(secret: &[u8]) -> Self {
        let secret_base64 = BASE64.encode(secret);
        let hash = Sha256::digest(secret_base64.as_bytes());
        let hash_string = format!("{hash:x}");
        let encoded = crockford_base32_encode_bytes(hash_string.as_bytes());
        let mut bytes = [0u8; Self::LEN];
        for (i, ch) in encoded.chars().take(Self::LEN).enumerate() {
            bytes[i] = ch as u8;
        }
        Self::from_encoded_bytes(bytes)
    }

    fn from_encoded_bytes(bytes: [u8; Self::LEN]) -> Self {
        debug_assert!(
            bytes.iter().all(|&b| is_crockford_char(b)),
            "encoded bytes must be valid Crockford symbols"
        );
        Self(bytes)
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: `parse` and `from_secret_bytes` only store ASCII Crockford symbols.
        std::str::from_utf8(&self.0).expect("valid utf-8")
    }
}

impl fmt::Display for DeviceDisplayCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn is_crockford_char(b: u8) -> bool {
    matches!(
        b,
        b'0'..=b'9'
            | b'A'..=b'H'
            | b'J'
            | b'K'
            | b'M'
            | b'N'
            | b'P'..=b'T'
            | b'V'..=b'Z'
    )
}

fn crockford_base32_encoding() -> data_encoding::Encoding {
    let mut spec = Specification::new();
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
    fn device_display_code_from_secret_matches_known_vector() {
        assert_eq!(
            DeviceDisplayCode::from_secret_bytes(&[0u8; 32]),
            DeviceDisplayCode::parse("6MRK").unwrap()
        );
    }

    #[test]
    fn device_display_code_parse_accepts_trim_and_case() {
        assert_eq!(
            DeviceDisplayCode::parse(" 6mrk ").unwrap(),
            DeviceDisplayCode::parse("6MRK").unwrap()
        );
    }

    #[test]
    fn device_display_code_parse_rejects_bad_input() {
        assert_eq!(
            DeviceDisplayCode::parse("").unwrap_err(),
            DeviceDisplayCodeError::WrongLength {
                expected: 4,
                actual: 0
            }
        );
        assert_eq!(
            DeviceDisplayCode::parse("6MR").unwrap_err(),
            DeviceDisplayCodeError::WrongLength {
                expected: 4,
                actual: 3
            }
        );
        assert_eq!(
            DeviceDisplayCode::parse("6MRKX").unwrap_err(),
            DeviceDisplayCodeError::WrongLength {
                expected: 4,
                actual: 5
            }
        );
        assert_eq!(
            DeviceDisplayCode::parse("6IRK").unwrap_err(),
            DeviceDisplayCodeError::InvalidCharacter { character: 'I' }
        );
        assert_eq!(
            DeviceDisplayCode::parse("😀😀😀😀").unwrap_err(),
            DeviceDisplayCodeError::NotAscii
        );
    }
}
