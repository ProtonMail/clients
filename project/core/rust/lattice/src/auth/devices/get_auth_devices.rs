use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract};

use super::LtAuthDevice;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthGetDevicesReq;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthGetDevicesRes {
    pub auth_devices: Vec<LtAuthDevice>,
}

impl LtContract for LtAuthGetDevicesReq {
    type Response = LtAuthGetDevicesRes;
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/devices"))
    }
}

impl AuthReq for LtAuthGetDevicesReq {}
