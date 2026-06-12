use serde::{Deserialize, Deserializer, Serialize};
use std::borrow::Cow;

use crate::{
    LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive, UnauthReq,
    auth::{
        LtAuthApiSession, LtAuthPasswordMode, LtAuthTwoFactorOptions,
        post_auth_2fa::{LtAuthSrpProof, LtAuthTwoFactorProof},
    },
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostReq {
    pub username: String,

    #[serde(flatten)]
    pub srp_proof: LtAuthSrpProof,

    #[serde(flatten)]
    pub tfa_proof: Option<LtAuthTwoFactorProof>,

    /// The client's fingerprint, for anti-abuse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostRes {
    #[serde(flatten)]
    pub session: LtAuthApiSession,
    pub server_proof: Sensitive<String>,
    pub password_mode: LtAuthPasswordMode,
    #[serde(rename = "2FA", default, deserialize_with = "deserialize_filtered_tfa")]
    pub tfa: Option<LtAuthTwoFactorOptions>,
}

fn deserialize_filtered_tfa<'de, D>(
    deserializer: D,
) -> Result<Option<LtAuthTwoFactorOptions>, D::Error>
where
    D: Deserializer<'de>,
{
    // Execute standard deserialization first
    let opt = Option::<LtAuthTwoFactorOptions>::deserialize(deserializer)?;

    // Filter out the state where enabled == 0
    Ok(opt.filter(|t| !t.enabled.is_empty()))
}

impl LtContract for LtAuthPostReq {
    type Response = LtSlimAPIJSON<LtAuthPostRes>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4"))
    }
}

impl UnauthReq for LtAuthPostReq {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostSsoReq {
    #[serde(rename = "SSOResponseToken")]
    pub sso_response_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostSsoRes {
    #[serde(flatten)]
    pub session: LtAuthApiSession,
}

impl LtContract for LtAuthPostSsoReq {
    type Response = LtSlimAPIJSON<LtAuthPostSsoRes>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4"))
    }
}

impl UnauthReq for LtAuthPostSsoReq {}
