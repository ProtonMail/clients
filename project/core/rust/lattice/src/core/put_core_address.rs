use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method,
    auth::LtAuthAddressId,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePutAddressBody {
    pub display_name: String,
    pub signature: String,
}

#[derive(Debug)]
pub struct LtCorePutAddressReq {
    pub id: LtAuthAddressId,
    pub body: LtCorePutAddressBody,
}

impl LtContract for LtCorePutAddressReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<&'a LtCorePutAddressBody>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(&self.body)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/core/v4/addresses/{}", self.id.0)))
    }
}

impl AuthReq for LtCorePutAddressReq {}
