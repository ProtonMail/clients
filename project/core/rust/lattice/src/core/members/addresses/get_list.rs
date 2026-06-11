//! `GET /core/v4/members/{memberID}/addresses` — list addresses for an org member (admin).

use std::borrow::Cow;
use std::num::NonZeroU32;

use crate::core::LtCoreAddress;
use crate::core::addresses::{LtCoreAddressesListQuery, LtCoreAddressesListRes};
use crate::core::ids::LtCoreMemberEncId;
use crate::{AuthReq, LatticeError, LtContract, LtPaginable, LtSerdeQueryParams, LtSlimAPIJSON};

/// Request to list addresses for a member (path `member_id` = encrypted member id).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetMembersMemberIDAddressesReq {
    pub member_id: LtCoreMemberEncId,
    pub query: LtCoreAddressesListQuery,
}

impl LtContract for LtCoreGetMembersMemberIDAddressesReq {
    type Response = LtSlimAPIJSON<LtCoreAddressesListRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtSerdeQueryParams<&'q LtCoreAddressesListQuery>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/members/{}/addresses",
            self.member_id
        )))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        Some(LtSerdeQueryParams(&self.query))
    }
}

impl LtPaginable for LtCoreGetMembersMemberIDAddressesReq {
    type Item = LtCoreAddress;
    const MAX_PAGE_SIZE: NonZeroU32 = NonZeroU32::new(150).unwrap();

    fn page_items(res: LtSlimAPIJSON<LtCoreAddressesListRes>) -> (Option<u32>, Vec<LtCoreAddress>) {
        (res.0.total, res.0.addresses)
    }
}

impl AuthReq for LtCoreGetMembersMemberIDAddressesReq {}
