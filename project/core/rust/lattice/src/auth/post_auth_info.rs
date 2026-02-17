use std::borrow::Cow;

use crate::{
    AuthReq, LatticeContract, LatticeError, Method, UnauthReq,
    auth::{LtAuthSrpChallenge, LtAuthTwoFactorOptions},
};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostInfoReq {
    pub username: Option<String>,
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

impl LatticeContract for LtAuthPostInfoReq {
    type Response = LtAuthPostInfoRes;
    type Body<'b> = &'b Self;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(self))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/info"))
    }
}

impl AuthReq for LtAuthPostInfoReq {}
impl UnauthReq for LtAuthPostInfoReq {}
