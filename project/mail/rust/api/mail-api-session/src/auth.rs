use serde_repr::{Deserialize_repr, Serialize_repr};

/// Re-export the key secret type.
pub use proton_crypto_account::salts::KeySecret;

/// Re-export the `mail_muon` auth type.
pub use mail_muon::client::{Auth, Tokens};

/// The password mode as reported by the API during authentication.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize_repr,
    Serialize_repr,
    Eq,
    Hash,
    PartialEq
)]
#[repr(u8)]
pub enum PasswordMode {
    One = 1,
    Two = 2,
}

/// The user key secret to unlock user keys.
#[derive(Debug, Clone)]
pub struct UserKeySecret(pub KeySecret);

impl UserKeySecret {
    /// Exposes the internal key secret to unlock user keys.
    #[must_use]
    pub fn expose_secret(&self) -> &KeySecret {
        &self.0
    }
}

impl<T: Into<Vec<u8>>> From<T> for UserKeySecret {
    fn from(value: T) -> Self {
        Self(KeySecret::new(value.into()))
    }
}
