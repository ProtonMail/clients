use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostDevicesCreateReq {
    pub name: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub activation_token: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostDevicesCreateRes {
    pub auth_device: LtAuthPostDevicesCreateAuthDevice,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostDevicesCreateAuthDevice {
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: String,

    pub device_token: String,
}

impl LtContract for LtAuthPostDevicesCreateReq {
    type Response = LtSlimAPIJSON<LtAuthPostDevicesCreateRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/devices"))
    }
}

impl AuthReq for LtAuthPostDevicesCreateReq {}
