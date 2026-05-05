use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtEmptyBody, LtNoQueryParams, LtSlimAPIJSON, Method,
};

/// `PUT /auth/v4/devices/{deviceId}/admin` — request org-admin help (no request body; `LtEmptyBody`).
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthPutDevicesDeviceIDAdminReq {
    /// Path only; not part of a JSON request body.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub device_id: String,
}

impl LtContract for LtAuthPutDevicesDeviceIDAdminReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtEmptyBody;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtEmptyBody))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/auth/v4/devices/{}/admin",
            self.device_id
        )))
    }
}

impl AuthReq for LtAuthPutDevicesDeviceIDAdminReq {}
