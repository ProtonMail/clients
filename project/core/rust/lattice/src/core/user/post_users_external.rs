use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{
    LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, UnauthReq,
    core::user::{LtCoreCreateUserType, LtCoreSrpVerifier, LtCoreUser},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePostUsersExternalReq {
    /// The type of user being created (e.g., internal or external).
    #[serde(rename = "Type")]
    pub user_type: LtCoreCreateUserType,

    /// The email address associated with the external user.
    pub email: String,

    /// The SRP verifier for user creation.
    pub auth: LtCoreSrpVerifier,

    /// The referrer for the user, if any.
    pub referrer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePostUsersExternalRes {
    /// The details of the newly created user.
    pub user: LtCoreUser,
}

impl LtContract for LtCorePostUsersExternalReq {
    type Response = LtSlimAPIJSON<LtCorePostUsersExternalRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users/external"))
    }
}

impl UnauthReq for LtCorePostUsersExternalReq {}
