//! `PUT /auth/v4/devices/{deviceId}/reject` — member rejects a pending auth device.

use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtEmptyBody, LtNoQueryParams, LtSlimAPIJSON, Method,
    core::LtCoreAuthDeviceId,
};

/// Path identifies device; no JSON body.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthPutDevicesDeviceIDRejectReq {
    pub device_id: LtCoreAuthDeviceId,
}

impl LtContract for LtAuthPutDevicesDeviceIDRejectReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtEmptyBody;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtEmptyBody))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/auth/v4/devices/{}/reject",
            self.device_id
        )))
    }
}

impl AuthReq for LtAuthPutDevicesDeviceIDRejectReq {}
