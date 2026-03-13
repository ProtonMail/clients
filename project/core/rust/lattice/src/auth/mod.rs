use std::fmt::Display;

use derive_more::{AsRef, Deref, Display, From, Into};
use passkey::types::webauthn::CredentialRequestOptions;
use zeroize::Zeroize;

use crate::Sensitive;

pub mod delete_auth;
pub mod devices;
pub mod get_auth_modulus;
pub mod get_auth_sessions_forks;
pub mod get_auth_sessions_forks_by_id;
pub mod get_password_policy;
pub mod post_auth;
pub mod post_auth_2fa;
pub mod post_auth_info;
pub mod post_sessions_forks;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthApiSession {
    #[cfg_attr(feature = "serde", serde(rename = "UID"))]
    pub id: LtAuthSessionId,
    #[cfg_attr(feature = "serde", serde(rename = "UserID"))]
    pub user_id: LtAuthUserId,
    /// EventID may be missing in fork responses
    #[cfg_attr(feature = "serde", serde(rename = "EventID"))]
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub event_id: Option<LtAuthEventId>,
    pub access_token: Sensitive<String>,
    pub refresh_token: Sensitive<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub scopes: Vec<String>,
}

#[derive(Into, From, Deref, AsRef)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Display, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthEventId(pub String);

#[derive(Into, From, Deref, AsRef)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Display, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthSessionId(pub String);

#[derive(Into, From, Deref, AsRef)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Display, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthUserId(pub String);

#[derive(Into, From, Deref, AsRef)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Display, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthAddressId(pub String);

#[derive(Zeroize)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthFidoKey {
    #[cfg_attr(feature = "serde", serde(rename = "CredentialID"))]
    pub credential_id: LtAuthFidoKeyId,
    pub attestation_format: String,
    pub name: String,
}

#[derive(Zeroize)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthFidoKeyId(pub Vec<u8>);

impl From<Vec<u8>> for LtAuthFidoKeyId {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl From<LtAuthFidoKeyId> for Vec<u8> {
    fn from(LtAuthFidoKeyId(id): LtAuthFidoKeyId) -> Self {
        id
    }
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthSrpChallenge {
    #[cfg_attr(feature = "serde", serde(rename = "SRPSession"))]
    pub session: String,
    pub version: u8,
    pub salt: Sensitive<String>,
    pub modulus: Sensitive<String>,
    pub server_ephemeral: Sensitive<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthTwoFactorOptions {
    pub enabled: LtAuthTwoFactorMethod,
    #[cfg_attr(feature = "serde", serde(rename = "FIDO2"))]
    pub fido: Option<LtAuthFidoOptions>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Copy)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthTwoFactorMethod(u8);

bitflags::bitflags! {
    impl LtAuthTwoFactorMethod: u8 {
        const TOTP = 1 << 0;
        const FIDO = 1 << 1;
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthFidoOptions {
    #[cfg_attr(feature = "serde", serde(rename = "AuthenticationOptions"))]
    pub options: Option<CredentialRequestOptions>,

    #[cfg_attr(feature = "serde", serde(rename = "RegisteredKeys"))]
    pub keys: Vec<LtAuthFidoKey>,
}

/// Definition: Password mode enum
#[repr(i32)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
#[cfg_attr(feature = "serde", serde(into = "i32"))]
#[cfg_attr(feature = "serde", serde(try_from = "i32"))]
pub enum LtAuthPasswordMode {
    One = 1,
    Two = 2,
}

impl TryFrom<i32> for LtAuthPasswordMode {
    type Error = LtAuthInvalidPasswordModeError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(LtAuthPasswordMode::One),
            2 => Ok(LtAuthPasswordMode::Two),
            _ => Err(LtAuthInvalidPasswordModeError),
        }
    }
}

impl From<LtAuthPasswordMode> for i32 {
    fn from(val: LtAuthPasswordMode) -> Self {
        val as i32
    }
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthInvalidPasswordModeError;

impl Display for LtAuthInvalidPasswordModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid password mode")
    }
}

impl std::error::Error for LtAuthInvalidPasswordModeError {}
