//! `GET /core/v4/members/{id}/devices` — list auth devices for an org member (admin).
//!
//! Source: `Proton\Apps\Account\Controller\Auth\GetAuthDevicesAction::getAuthDevicesFromAdmin`. Scope: `FULL` | `ORGANIZATION`.
//! Rows use [`crate::auth::devices::LtAuthDevice`] (Core’s `core` feature enables `auth`).

use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::auth::devices::LtAuthDevice;
use crate::core::ids::LtCoreMemberEncId;
use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON};

/// Request to list auth devices for a member (path `id` = encrypted member id).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetMembersDevicesReq {
    pub member_id: LtCoreMemberEncId,
}

/// Response body fields beside `Code` (key `AuthDevices`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCoreGetMembersDevicesRes {
    #[serde(rename = "AuthDevices")]
    pub auth_devices: Vec<LtAuthDevice>,
}

impl LtContract for LtCoreGetMembersDevicesReq {
    type Response = LtSlimAPIJSON<LtCoreGetMembersDevicesRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/members/{}/devices",
            self.member_id
        )))
    }
}

impl AuthReq for LtCoreGetMembersDevicesReq {}
