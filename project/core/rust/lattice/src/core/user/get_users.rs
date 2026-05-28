use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, core::user::LtCoreUser,
};

/// Request current user info (`GET /core/v4/users`).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetUsersReq;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetUsersRes {
    pub user: LtCoreUser,
}

impl LtContract for LtCoreGetUsersReq {
    type Response = LtSlimAPIJSON<LtCoreGetUsersRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users"))
    }
}

impl AuthReq for LtCoreGetUsersReq {}
