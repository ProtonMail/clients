use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive,
    auth::LtAuthFidoKeyId, core::user::LtCoreSrpVerifier,
};

/// Inline FIDO2 payload for `PUT /core/v4/keys/private` (alternative to `TwoFactorCode`).
///
/// `AuthenticationOptions` is the same JSON object returned by the server as the WebAuthn challenge;
/// assertion fields are the values from the client authentication library (typically base64 on the wire).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePutKeysPrivateFido2Input {
    #[serde(rename = "AuthenticationOptions")]
    pub authentication_options: serde_json::Value,
    pub client_data: Sensitive<String>,
    pub authenticator_data: Sensitive<String>,
    pub signature: Sensitive<String>,
    #[serde(rename = "CredentialID")]
    pub credential_id: LtAuthFidoKeyId,
}

/// One private key entry for `PUT /core/v4/keys/private`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePutKeysPrivateKeyEntry {
    #[serde(rename = "ID")]
    pub id: String,
    pub private_key: Sensitive<String>,
}

/// Request for `PUT /core/v4/keys/private` (mailbox / single password change, SSO backup password, etc.).
///
/// Updates re-encrypted private keys only; does not activate keys you cannot unlock — use “Activate Key” first.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePutKeysPrivateReq {
    pub key_salt: Sensitive<String>,

    pub keys: Vec<LtCorePutKeysPrivateKeyEntry>,

    pub user_keys: Vec<LtCorePutKeysPrivateKeyEntry>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Organization private key (armored). Required for org admins (legacy scheme).
    pub organization_key: Option<Sensitive<String>>,

    /// New SRP verifier (`AuthInput`) for the updated password.
    pub auth: LtCoreSrpVerifier,

    /// Optional: inline re-authentication (password change with active session proof).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ephemeral: Option<Sensitive<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_proof: Option<Sensitive<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "SRPSession")]
    pub srp_session: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub two_factor_code: Option<String>,

    /// Optional: inline re-authentication via FIDO2 (alternative to `TwoFactorCode`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "FIDO2")]
    pub fido2: Option<LtCorePutKeysPrivateFido2Input>,

    /// Required for SSO sessions: base64 AES-GCM encrypted passphrase using `DeviceSecret`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_secret: Option<Sensitive<String>>,
}

/// Response body for `PUT /core/v4/keys/private` (wrapped by SlimAPI `Code` + flattened body).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePutKeysPrivateRes {
    /// Present only when inline re-authentication fields were submitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_proof: Option<Sensitive<String>>,
}

impl LtContract for LtCorePutKeysPrivateReq {
    type Response = LtSlimAPIJSON<LtCorePutKeysPrivateRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/keys/private"))
    }
}

impl AuthReq for LtCorePutKeysPrivateReq {}
