use std::borrow::Cow;

use crate::{LatticeError, LtContract, Method, UnauthReq};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePostValidateEmailReq {
    pub email: String,
}

impl LtContract for LtCorePostValidateEmailReq {
    type Response = ();
    type Body<'a> = &'a Self;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(self))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/validate/email"))
    }
}

impl UnauthReq for LtCorePostValidateEmailReq {}
