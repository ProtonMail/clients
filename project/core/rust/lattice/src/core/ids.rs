//! Opaque Core API identifiers (serialized as plain strings).

use derive_more::{AsRef, Deref, Display, From, Into};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Encrypted domain identifier (`ID` on domain DTOs, path segment for `/core/v4/domains/{id}`).
#[derive(
    Into, From, Deref, AsRef, Debug, Display, Clone, PartialEq, Eq, Hash, Default
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct LtCoreDomainId(pub String);

/// Encrypted organization member identifier (`ID` on member rows, path segment for SAML routes).
#[derive(
    Into, From, Deref, AsRef, Debug, Display, Clone, PartialEq, Eq, Hash, Default
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct LtCoreMemberEncId(pub String);

/// Opaque auth-device identifier (path segment for `/core/v4/members/.../devices/{deviceId}/...`, `AuthDeviceID` in JSON).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[derive(
    Into, From, Deref, AsRef, Debug, Display, Clone, PartialEq, Eq, Hash, Default
)]
pub struct LtCoreAuthDeviceId(pub String);
