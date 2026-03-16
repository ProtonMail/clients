use std::borrow::Cow;

use crate::{LatticeError, LtContract, UnauthReq};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetTestsPingReq;

impl LtContract for LtCoreGetTestsPingReq {
    type Response = ();
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/tests/ping"))
    }
}

impl UnauthReq for LtCoreGetTestsPingReq {}
