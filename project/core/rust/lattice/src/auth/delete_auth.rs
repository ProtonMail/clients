use std::borrow::Cow;

use crate::{AuthReq, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthDeleteReq;

impl LtContract for LtAuthDeleteReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, crate::LatticeError> {
        Ok(Cow::Borrowed("/auth/v4"))
    }

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, crate::LatticeError> {
        Ok(Method::Delete)
    }
}

impl AuthReq for LtAuthDeleteReq {}
