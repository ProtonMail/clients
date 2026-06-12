use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method,
    core::LtCoreAuthDeviceId,
};

use super::LtAuthAssociatedDevice;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostDevicesAssociateReq {
    #[serde(skip)]
    pub device_id: LtCoreAuthDeviceId,
    pub device_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostDevicesAssociateRes {
    pub auth_device: LtAuthAssociatedDevice,
}

impl LtContract for LtAuthPostDevicesAssociateReq {
    type Response = LtSlimAPIJSON<LtAuthPostDevicesAssociateRes>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/auth/v4/devices/{}/associate",
            self.device_id
        )))
    }
}

impl AuthReq for LtAuthPostDevicesAssociateReq {}
