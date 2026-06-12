use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive,
    auth::post_auth_2fa::{LtAuthSrpProof, LtAuthTwoFactorProof},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePutUsersPasswordRes {
    pub server_proof: Sensitive<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePutUsersPasswordReq {
    #[serde(flatten)]
    pub srp_proof: LtAuthSrpProof,

    #[serde(flatten)]
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
