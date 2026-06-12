use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{
    LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, UnauthReq,
    core::user::{LtCoreCreateUserType, LtCoreSrpVerifier, LtCoreUser},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePostUsersReq {
    /// The type of user being created (e.g., internal or external).
    #[serde(rename = "Type")]
    pub user_type: LtCoreCreateUserType,

    /// The username to be created.
    pub username: String,

    /// The domain for the user, if applicable.
    pub domain: Option<String>,

    /// The SRP verifier for user creation.
    pub auth: LtCoreSrpVerifier,

    /// The email address associated with the user.
    pub email: Option<String>,

    /// The phone number associated with the user.
    pub phone: Option<String>,

    /// The referrer for the user, if any.
    pub referrer: Option<String>,

    /// The challenge payload, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePostUsersRes {
    /// The details of the newly created user.
    pub user: LtCoreUser,
}

impl LtContract for LtCorePostUsersReq {
    type Response = LtSlimAPIJSON<LtCorePostUsersRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users"))
    }
}

impl UnauthReq for LtCorePostUsersReq {}
