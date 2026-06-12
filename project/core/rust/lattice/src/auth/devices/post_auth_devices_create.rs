use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use core_sensitive_data::Sensitive;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

use super::LtAuthDevice;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostDevicesCreateReq {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation_token: Option<Sensitive<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthPostDevicesCreateRes {
    pub auth_device: LtAuthDevice,
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
