//! Unprivatization (SSO org member) DTOs: `MagicLinkService` / `GetMemberUnprivatizationOutput` in Account.
//! **Placement:** one module under [`crate::core`], next to the route types that embed these structs (not a
//! separate crate or one file per newtype — the surface is small and stable).
//!
//! **Names:** `LtCoreUnpriv*` keeps call sites readable (shorter than `LtCoreUnprivatization*`).
//!
//! **Sensitivity:** [`Sensitive`] wraps armored PGP **signatures**, **private** key material, and
//! **activation** payloads (redaction/logging). Plain `String` is for JSON [`LtCoreUnprivInvitationData`]
//! and **public** [`LtCoreUnprivPgpPublicKey`].

use derive_more::{Deref, From};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};

use crate::Sensitive;

/// Definition: `bundles/AccountInternalBundle/src/Domain/Organization/UnprivatizationState.php`
/// (`0` = Declined, `1` = Pending, `2` = Ready).
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum LtCoreUnprivState {
    Declined = 0,
    Pending = 1,
    Ready = 2,
}

/// JSON string: must match signed bytes in admin `POST .../unprivatize` and member views.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, From, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LtCoreUnprivInvitationData(pub String);

/// Detached PGP armored signature over `InvitationData` with context
/// `account.unprivatization-invitation-data` (org public key).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, From, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LtCoreUnprivInvitationSignature(pub Sensitive<String>);

/// Armored PGP public key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, From, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LtCoreUnprivPgpPublicKey(pub String);

/// Armored PGP signature over the org public key’s SHA-256 hex fingerprint; context
/// `account.organization-fingerprint` (signatures use [`Sensitive`] like [`LtCoreUnprivInvitationSignature`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, From, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LtCoreUnprivOrgKeyFingerprintSignature(pub Sensitive<String>);

/// First armored private-key block on the list embed (duplicate of `PrivateKeys[0]` when both set).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, From, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LtCoreUnprivArmoredPrivateKey(pub Sensitive<String>);

/// Armored PGP “activation” payload on the list embed, when set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, From, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LtCoreUnprivActivationToken(pub Sensitive<String>);
