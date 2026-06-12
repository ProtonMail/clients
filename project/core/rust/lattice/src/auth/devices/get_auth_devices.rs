use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON};

use super::LtAuthDevice;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtAuthGetDevicesReq;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthGetDevicesRes {
    pub auth_devices: Vec<LtAuthDevice>,
}

impl LtContract for LtAuthGetDevicesReq {
    type Response = LtSlimAPIJSON<LtAuthGetDevicesRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/devices"))
    }
}

impl AuthReq for LtAuthGetDevicesReq {}
