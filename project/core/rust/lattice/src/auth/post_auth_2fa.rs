use std::borrow::Cow;

use passkey::types::webauthn::CredentialRequestOptions;

use crate::auth::LtAuthFidoKeyId;
use crate::{AuthReq, LatticeError, LtContract, Method, Sensitive};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostSessionsForks {
    /// The client ID of the child session
    #[cfg_attr(feature = "serde", serde(rename = "ChildClientID"))]
    pub child_client_id: String,
    /// Whether the child session should be independent
    #[cfg_attr(feature = "serde", serde(with = "crate::helpers::bool_opt_int"))]
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub independent: Option<bool>,
    /// Base64-encoded encrypted payload
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub payload: Option<String>,
    /// The user code (for QR login)
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub user_code: Option<String>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthSrpProof {
    #[cfg_attr(feature = "serde", serde(rename = "SRPSession"))]
    pub srp_session: String,
    pub client_ephemeral: Sensitive<String>,
    pub client_proof: Sensitive<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub enum LtAuthTwoFactorProof {
    #[cfg_attr(feature = "serde", serde(rename = "TwoFactorCode"))]
    Totp(Sensitive<String>),

    #[cfg_attr(feature = "serde", serde(rename = "FIDO2"))]
    Fido {
        #[cfg_attr(feature = "serde", serde(flatten))]
        assertion: LtAuthFidoAssertion,

        #[cfg_attr(feature = "serde", serde(rename = "AuthenticationOptions"))]
        options: Box<CredentialRequestOptions>,
    },
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthFidoAssertion {
    #[cfg_attr(feature = "serde", serde(rename = "CredentialID"))]
    pub credential_id: LtAuthFidoKeyId,
    pub client_data: Sensitive<String>,
    pub authenticator_data: Sensitive<String>,
    pub signature: Sensitive<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPost2fa {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub tfa_proof: LtAuthTwoFactorProof,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPost2faRes {
    pub scopes: Vec<String>,
}

impl LtContract for LtAuthPost2fa {
    type Response = LtAuthPost2faRes;
    type Body<'b> = &'b Self;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(self))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/2fa"))
    }
}

impl AuthReq for LtAuthPost2fa {}
