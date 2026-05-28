//! `GET /core/v4/members/devices/pending` — auth devices in the org with a non-null activation token.
//!
//! Source: `Proton\Apps\Account\Controller\Auth\GetPendingAuthDevicesAction`. Scope: `ORGANIZATION` only.
//! Wire: top-level key `MemberAuthDevices` with flat [`crate::auth::devices::LtAuthDevice`] elements.

use std::borrow::Cow;

use crate::auth::devices::LtAuthDevice;
use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON};

/// No path or query parameters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LtCoreGetMembersDevicesPendingReq;

/// Response body fields beside `Code` (key `MemberAuthDevices` per server).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetMembersDevicesPendingRes {
    #[cfg_attr(feature = "serde", serde(rename = "MemberAuthDevices"))]
    pub member_auth_devices: Vec<LtAuthDevice>,
}

impl LtContract for LtCoreGetMembersDevicesPendingReq {
    type Response = LtSlimAPIJSON<LtCoreGetMembersDevicesPendingRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/members/devices/pending"))
    }
}

impl AuthReq for LtCoreGetMembersDevicesPendingReq {}
