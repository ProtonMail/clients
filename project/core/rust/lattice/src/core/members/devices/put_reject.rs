//! `PUT /core/v4/members/{memberId}/devices/{deviceId}/reject` — org admin rejects a pending auth device.
//!
//! Source: `Proton\Apps\Account\Controller\Auth\RejectAuthDeviceAction::rejectAuthDevicesFromAdmin`. Scope: `FULL` | `ORGANIZATION`.

use std::borrow::Cow;

use crate::core::ids::{LtCoreAuthDeviceId, LtCoreMemberEncId};
use crate::{
    AuthReq, LatticeError, LtContract, LtEmptyBody, LtNoQueryParams, LtSlimAPIJSON, Method,
};

/// Path identifies member and device; no JSON body.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCorePutMembersDevicesRejectReq {
    pub member_id: LtCoreMemberEncId,
    pub device_id: LtCoreAuthDeviceId,
}

impl LtContract for LtCorePutMembersDevicesRejectReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtEmptyBody;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtEmptyBody))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/members/{}/devices/{}/reject",
            self.member_id, self.device_id
        )))
    }
}

impl AuthReq for LtCorePutMembersDevicesRejectReq {}
