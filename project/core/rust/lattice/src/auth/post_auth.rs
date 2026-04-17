use std::borrow::Cow;

use crate::{
    LatticeError, LtContract, LtSlimAPIJSON, Method, Sensitive, UnauthReq,
    auth::{
        LtAuthApiSession, LtAuthPasswordMode, LtAuthTwoFactorOptions,
        post_auth_2fa::{LtAuthSrpProof, LtAuthTwoFactorProof},
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostReq {
    pub username: String,

    #[cfg_attr(feature = "serde", serde(flatten))]
    pub srp_proof: LtAuthSrpProof,

    #[cfg_attr(feature = "serde", serde(flatten))]
    pub tfa_proof: Option<LtAuthTwoFactorProof>,

    /// The client's fingerprint, for anti-abuse.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg(feature = "serde")]
    pub payload: Option<serde_json::Value>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostRes {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub session: LtAuthApiSession,
    pub server_proof: Sensitive<String>,
    pub password_mode: LtAuthPasswordMode,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "2FA", default, deserialize_with = "deserialize_filtered_tfa")
    )]
    pub tfa: Option<LtAuthTwoFactorOptions>,
}

#[cfg(feature = "serde")]
fn deserialize_filtered_tfa<'de, D>(
    deserializer: D,
) -> Result<Option<LtAuthTwoFactorOptions>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    // Execute standard deserialization first
    let opt = Option::<LtAuthTwoFactorOptions>::deserialize(deserializer)?;

    // Filter out the state where enabled == 0
    Ok(opt.filter(|t| !t.enabled.is_empty()))
}

impl LtContract for LtAuthPostReq {
    type Response = LtSlimAPIJSON<LtAuthPostRes>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4"))
    }
}

impl UnauthReq for LtAuthPostReq {}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostSsoReq {
    #[cfg_attr(feature = "serde", serde(rename = "SSOResponseToken"))]
    pub sso_response_token: String,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostSsoRes {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub session: LtAuthApiSession,
}

impl LtContract for LtAuthPostSsoReq {
    type Response = LtSlimAPIJSON<LtAuthPostSsoRes>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4"))
    }
}

impl UnauthReq for LtAuthPostSsoReq {}
