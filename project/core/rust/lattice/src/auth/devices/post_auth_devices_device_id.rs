use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive,
    core::LtCoreAuthDeviceId,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostDevicesDeviceIDReq {
    #[serde(skip)]
    pub device_id: LtCoreAuthDeviceId,
    pub encrypted_secret: Sensitive<String>,
}

impl LtContract for LtAuthPostDevicesDeviceIDReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/auth/v4/devices/{}", self.device_id)))
    }
}

impl AuthReq for LtAuthPostDevicesDeviceIDReq {}
