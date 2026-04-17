use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON, Method};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthPutDevicesDeviceIDAdminReq {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub device_id: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtAuthPutDevicesAdminRes;

impl LtContract for LtAuthPutDevicesDeviceIDAdminReq {
    type Response = LtSlimAPIJSON<LtAuthPutDevicesAdminRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/auth/v4/devices/{}/admin",
            self.device_id
        )))
    }
}

impl AuthReq for LtAuthPutDevicesDeviceIDAdminReq {}
