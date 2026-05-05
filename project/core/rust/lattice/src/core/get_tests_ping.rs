use std::borrow::Cow;

use crate::{LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, UnauthReq};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetTestsPingReq;

impl LtContract for LtCoreGetTestsPingReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/tests/ping"))
    }
}

impl UnauthReq for LtCoreGetTestsPingReq {}
