use std::borrow::Cow;

use crate::{LatticeError, LtContract, UnauthReq};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthGetSessionsForksReq;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthGetSessionsForksRes {
    /// Random 20-char selector for polling
    pub selector: String,

    /// Random 8-char user code for display
    pub user_code: String,
}

impl LtContract for LtAuthGetSessionsForksReq {
    type Response = LtAuthGetSessionsForksRes;
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/sessions/forks"))
    }
}

impl UnauthReq for LtAuthGetSessionsForksReq {}
