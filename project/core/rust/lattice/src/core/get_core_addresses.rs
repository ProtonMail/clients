use std::borrow::Cow;
use std::num::NonZeroU32;

use crate::{
    AuthReq, LatticeError, LtContract, LtPaginable, LtSerdeQueryParams, LtSlimAPIJSON,
    core::LtCoreAddress,
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

impl LtPaginable for LtCoreGetAddressesReq {
    type Item = LtCoreAddress;
    const MAX_PAGE_SIZE: NonZeroU32 = NonZeroU32::new(150).unwrap();

    fn page_items(res: LtSlimAPIJSON<LtCoreAddressesListRes>) -> (Option<u32>, Vec<LtCoreAddress>) {
        (res.0.total, res.0.addresses)
    }
}

impl AuthReq for LtCoreGetAddressesReq {}
