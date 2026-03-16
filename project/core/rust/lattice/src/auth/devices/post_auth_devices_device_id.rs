use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, Method, Sensitive};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostDevicesDeviceIDReq {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub device_id: String,
    pub encrypted_secret: Sensitive<String>,
}

impl LtContract for LtAuthPostDevicesDeviceIDReq {
    type Response = ();
    type Body<'b> = &'b Self;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(self))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/auth/v4/devices/{}", self.device_id)))
    }
}

impl AuthReq for LtAuthPostDevicesDeviceIDReq {}
