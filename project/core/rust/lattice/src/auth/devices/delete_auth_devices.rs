use std::borrow::Cow;

use crate::{AuthReq, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum LtAuthDeleteDevicesReq {
    All,
    DeviceID(String),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthDeleteDevicesRes {}

impl LtContract for LtAuthDeleteDevicesReq {
    type Response = LtSlimAPIJSON<LtAuthDeleteDevicesRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, crate::LatticeError> {
        match self {
            Self::All => Ok(Cow::Borrowed("/auth/v4/devices")),
            Self::DeviceID(device_id) => Ok(Cow::Owned(format!("/auth/v4/devices/{}", device_id))),
        }
    }

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, crate::LatticeError> {
        Ok(Method::Delete)
    }
}

impl AuthReq for LtAuthDeleteDevicesReq {}
