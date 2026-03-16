use std::borrow::Cow;

use crate::{AuthReq, LtContract, Method};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostSessionsForksReq {
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
pub struct LtAuthPostSessionsForksRes {
    /// The selector for the created fork
    pub selector: String,
}

impl LtContract for LtAuthPostSessionsForksReq {
    type Response = LtAuthPostSessionsForksRes;
    type Body<'a> = &'a Self;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, crate::LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/sessions/forks"))
    }

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, crate::LatticeError> {
        Ok(Method::Post(self))
    }
}

impl AuthReq for LtAuthPostSessionsForksReq {}
