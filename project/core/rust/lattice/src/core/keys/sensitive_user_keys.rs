//! Zeroizing wrappers for crypto key types.
//!
//! These wrappers provide `Zeroize` implementations for key types from
//! `proton-crypto-account` that don't natively support it.

use proton_crypto_account::keys::{AddressKeys, UserKeys};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Deref, DerefMut};
use zeroize::Zeroize;

/// A wrapper around `UserKeys` that implements `Zeroize` and redacts debug output.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LtCoreSensitiveUserKeys(pub UserKeys);

impl LtCoreSensitiveUserKeys {
    /// Creates a new `SensitiveUserKeys` wrapper.
    pub fn new(keys: UserKeys) -> Self {
        Self(keys)
    }

    /// Consumes the wrapper and returns the inner keys.
    pub fn into_inner(self) -> UserKeys {
        self.0
    }
}

impl Zeroize for LtCoreSensitiveUserKeys {
    fn zeroize(&mut self) {
        use proton_crypto_account::keys::LockedKey;

        for locked_key in &mut self.0.0 {
            // Destructure to ensure compile error if LockedKey gains new fields.
            // If you're here because of a compile error, add zeroization for the new field!
            let LockedKey {
                id,
                version: _, // u32 - no sensitive data
                private_key,
                token,
                signature,
                activation,
                primary: _, // bool - no sensitive data
                active: _,  // bool - no sensitive data
                flags: _,   // Option<KeyFlag> - u32 bitmap, no sensitive data
                recovery_secret,
                recovery_secret_signature,
                address_forwarding_id: _, // identifier, not secret
            } = locked_key;

            private_key.0.zeroize();

            if let Some(t) = token {
                t.0.zeroize();
            }

            if let Some(secret) = recovery_secret {
                secret.zeroize();
            }

            if let Some(sig) = recovery_secret_signature {
                sig.zeroize();
            }

            if let Some(act) = activation {
                act.zeroize();
            }

            if let Some(sig) = signature {
                sig.0.zeroize();
            }

            id.0.zeroize();
        }

        self.0.0.clear();
        self.0.0.shrink_to_fit();
    }
}

impl Deref for LtCoreSensitiveUserKeys {
    type Target = UserKeys;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LtCoreSensitiveUserKeys {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<UserKeys> for LtCoreSensitiveUserKeys {
    fn from(keys: UserKeys) -> Self {
        Self(keys)
    }
}

impl AsRef<UserKeys> for LtCoreSensitiveUserKeys {
    fn as_ref(&self) -> &UserKeys {
        &self.0
    }
}

impl fmt::Debug for LtCoreSensitiveUserKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "redacted")
    }
}

/// A wrapper around `AddressKeys` that implements `Zeroize` and redacts debug output.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LtCoreSensitiveAddressKeys(pub AddressKeys);

impl LtCoreSensitiveAddressKeys {
    /// Creates a new `SensitiveAddressKeys` wrapper.
    pub fn new(keys: AddressKeys) -> Self {
        Self(keys)
    }

    /// Consumes the wrapper and returns the inner keys.
    pub fn into_inner(self) -> AddressKeys {
        self.0
    }
}

impl Zeroize for LtCoreSensitiveAddressKeys {
    fn zeroize(&mut self) {
        use proton_crypto_account::keys::LockedKey;

        for locked_key in &mut self.0.0 {
            // Destructure to ensure compile error if LockedKey gains new fields.
            // If you're here because of a compile error, add zeroization for the new field!
            let LockedKey {
                id,
                version: _, // u32 - no sensitive data
                private_key,
                token,
                signature,
                activation,
                primary: _, // bool - no sensitive data
                active: _,  // bool - no sensitive data
                flags: _,   // Option<KeyFlag> - u32 bitmap, no sensitive data
                recovery_secret,
                recovery_secret_signature,
                address_forwarding_id: _, // identifier, not secret
            } = locked_key;

            private_key.0.zeroize();

            if let Some(t) = token {
                t.0.zeroize();
            }

            if let Some(secret) = recovery_secret {
                secret.zeroize();
            }

            if let Some(sig) = recovery_secret_signature {
                sig.zeroize();
            }

            if let Some(act) = activation {
                act.zeroize();
            }

            if let Some(sig) = signature {
                sig.0.zeroize();
            }

            id.0.zeroize();
        }

        self.0.0.clear();
        self.0.0.shrink_to_fit();
    }
}

impl Deref for LtCoreSensitiveAddressKeys {
    type Target = AddressKeys;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LtCoreSensitiveAddressKeys {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<AddressKeys> for LtCoreSensitiveAddressKeys {
    fn from(keys: AddressKeys) -> Self {
        Self(keys)
    }
}

impl AsRef<AddressKeys> for LtCoreSensitiveAddressKeys {
    fn as_ref(&self) -> &AddressKeys {
        &self.0
    }
}

impl fmt::Debug for LtCoreSensitiveAddressKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "redacted")
    }
}
