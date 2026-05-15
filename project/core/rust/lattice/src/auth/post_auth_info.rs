use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, UnauthReq,
    auth::{LtAuthSrpChallenge, LtAuthTwoFactorOptions},
};

#[repr(C)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LtAuthPostInfoIntent {
    Proton,
    #[cfg_attr(feature = "serde", serde(rename = "SSO"))]
    Sso,
    Auto,
}

/// `ReauthScope` on `POST /auth/v4/info` when a session is present: `password` or `locked` (lowercase in JSON).
#[repr(C)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum LtAuthReauthScope {
    Password,
    Locked,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostInfoReq {
    pub username: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub client_secret: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub intent: Option<LtAuthPostInfoIntent>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub is_testing: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub reauth_scope: Option<LtAuthReauthScope>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase", untagged))]
pub enum LtAuthPostInfoRes {
    SrpChallenge {
        username: Option<String>,

        #[cfg_attr(feature = "serde", serde(flatten))]
        srp_challenge: LtAuthSrpChallenge,

        #[cfg_attr(feature = "serde", serde(rename = "2FA"))]
        tfa: Box<Option<LtAuthTwoFactorOptions>>,
    },
    SsoChallenge {
        #[cfg_attr(feature = "serde", serde(rename = "SSOChallengeToken"))]
        sso_challenge_token: String,
    },
}

impl LtContract for LtAuthPostInfoReq {
    type Response = LtSlimAPIJSON<LtAuthPostInfoRes>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/info"))
    }
}

impl AuthReq for LtAuthPostInfoReq {}
impl UnauthReq for LtAuthPostInfoReq {}
