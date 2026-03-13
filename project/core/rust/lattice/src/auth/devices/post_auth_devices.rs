use std::borrow::Cow;

use crate::{AuthReq, LatticeContract, LatticeError, Method, auth::devices::LtAuthDevice};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostDevicesReq {
    /// User-facing device name
    pub name: String,
    /// Optional. If the user is already set-up, a 32-byte random token encoded as base64 and encrypted to the primary address key
    // #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub activation_token: Option<String>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPostDevicesRes {
    pub device: LtAuthDevice,
}

impl LatticeContract for LtAuthPostDevicesReq {
    type Response = LtAuthPostDevicesRes;
    type Body<'b> = &'b Self;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(self))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/devices"))
    }
}

impl AuthReq for LtAuthPostDevicesReq {}
