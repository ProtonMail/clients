use std::borrow::Cow;

use crate::{AuthReq, LatticeContract, LatticeError, core::user::LtCoreUser};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetUsersReq;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetUsersRes {
    pub user: LtCoreUser,
}

impl LatticeContract for LtCoreGetUsersReq {
    type Response = LtCoreGetUsersRes;
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users"))
    }
}

impl AuthReq for LtCoreGetUsersReq {}
