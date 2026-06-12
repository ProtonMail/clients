use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{AuthReq, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostSessionsForksReq {
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
pub struct LtAuthPostSessionsForksRes {
    /// The selector for the created fork
    pub selector: String,
}

impl LtContract for LtAuthPostSessionsForksReq {
    type Response = LtSlimAPIJSON<LtAuthPostSessionsForksRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, crate::LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/sessions/forks"))
    }

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, crate::LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }
}

impl AuthReq for LtAuthPostSessionsForksReq {}
