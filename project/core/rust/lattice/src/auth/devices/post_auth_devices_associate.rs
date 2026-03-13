use std::borrow::Cow;

use crate::{AuthReq, LatticeContract, LatticeError, Method};

use super::LtAuthAssociatedDevice;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostDevicesAssociateReq {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub device_id: String,
    pub device_token: String,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostDevicesAssociateRes {
    pub device: LtAuthAssociatedDevice,
}

impl LatticeContract for LtAuthPostDevicesAssociateReq {
    type Response = LtAuthPostDevicesAssociateRes;
    type Body<'b> = &'b Self;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(self))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/auth/v4/devices/{}/associate",
            self.device_id
        )))
    }
}

impl AuthReq for LtAuthPostDevicesAssociateReq {}
