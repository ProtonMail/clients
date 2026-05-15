use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive,
    auth::post_auth_2fa::{LtAuthSrpProof, LtAuthTwoFactorProof},
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePutUsersPasswordRes {
    pub server_proof: Sensitive<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePutUsersPasswordReq {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub srp_proof: LtAuthSrpProof,

    #[cfg_attr(feature = "serde", serde(flatten))]
    pub tfa_proof: Option<LtAuthTwoFactorProof>,
}

impl LtContract for LtCorePutUsersPasswordReq {
    type Response = LtSlimAPIJSON<LtCorePutUsersPasswordRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users/password"))
    }
}

impl AuthReq for LtCorePutUsersPasswordReq {}
