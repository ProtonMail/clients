use std::borrow::Cow;

use crate::{LatticeContract, LatticeError, Method, UnauthReq};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePostValidatePhoneReq {
    pub phone: String,
}

impl LatticeContract for LtCorePostValidatePhoneReq {
    type Response = ();
    type Body<'a> = &'a Self;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(self))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/validate/phone"))
    }
}

impl UnauthReq for LtCorePostValidatePhoneReq {}
