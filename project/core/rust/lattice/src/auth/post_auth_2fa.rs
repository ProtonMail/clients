use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use passkey::types::webauthn::CredentialRequestOptions;

use crate::auth::LtAuthFidoKeyId;
use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostSessionsForks {
    /// The client ID of the child session
    #[serde(rename = "ChildClientID")]
    pub child_client_id: String,
    /// Whether the child session should be independent
    #[serde(with = "crate::helpers::bool_opt_int")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub independent: Option<bool>,
    /// Base64-encoded encrypted payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// The user code (for QR login)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthSrpProof {
    #[serde(rename = "SRPSession")]
    pub srp_session: String,
    pub client_ephemeral: Sensitive<String>,
    pub client_proof: Sensitive<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LtAuthTwoFactorProof {
    #[serde(rename = "TwoFactorCode")]
    Totp(Sensitive<String>),

    #[serde(rename = "FIDO2")]
    Fido {
        #[serde(flatten)]
        assertion: LtAuthFidoAssertion,

        #[serde(rename = "AuthenticationOptions")]
        options: Box<CredentialRequestOptions>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthFidoAssertion {
    #[serde(rename = "CredentialID")]
    pub credential_id: LtAuthFidoKeyId,
    pub client_data: Sensitive<String>,
    pub authenticator_data: Sensitive<String>,
    pub signature: Sensitive<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPost2fa {
    #[serde(flatten)]
    pub tfa_proof: LtAuthTwoFactorProof,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPost2faRes {
    pub scopes: Vec<String>,
}

impl LtContract for LtAuthPost2fa {
    type Response = LtSlimAPIJSON<LtAuthPost2faRes>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/2fa"))
    }
}

impl AuthReq for LtAuthPost2fa {}
