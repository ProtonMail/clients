use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtSerdeQueryParams, LtSlimAPIJSON,
    core::addresses::{LtCoreAddressesListQuery, LtCoreAddressesListRes},
};

/// Request to list the authenticated user's addresses (`FULL` scope).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct LtCoreGetAddressesReq {
    pub query: LtCoreAddressesListQuery,
}

impl LtContract for LtCoreGetAddressesReq {
    type Response = LtSlimAPIJSON<LtCoreAddressesListRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtSerdeQueryParams<&'q LtCoreAddressesListQuery>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/addresses"))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        Some(LtSerdeQueryParams(&self.query))
    }
}

impl AuthReq for LtCoreGetAddressesReq {}
