//! Opaque Core API identifiers (serialized as plain strings).

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Encrypted domain identifier (`ID` on domain DTOs, path segment for `/core/v4/domains/{id}`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct LtCoreDomainId(pub String);

impl std::fmt::Display for LtCoreDomainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for LtCoreDomainId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for LtCoreDomainId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for LtCoreDomainId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Encrypted organization member identifier (`ID` on member rows, path segment for SAML routes).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct LtCoreMemberEncId(pub String);

impl std::fmt::Display for LtCoreMemberEncId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for LtCoreMemberEncId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for LtCoreMemberEncId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for LtCoreMemberEncId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
