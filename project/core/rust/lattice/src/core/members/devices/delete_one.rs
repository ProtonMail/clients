//! `DELETE /core/v4/members/{memberId}/devices/{deviceId}` — org admin removes one auth device.
//!
//! Source: `Proton\Apps\Account\Controller\Auth\DeleteAuthDeviceAction::deleteAdmin`. Scope: `ORGANIZATION`.
//! Distinct from [`crate::core::LtCoreDeleteMembersDevicesReq`] (bulk) and from MobileDevice `DELETE /core/v4/devices`.

use std::borrow::Cow;

use crate::core::ids::{LtCoreAuthDeviceId, LtCoreMemberEncId};
use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON, Method};

/// No request body.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreDeleteMembersDeviceReq {
    pub member_id: LtCoreMemberEncId,
    pub device_id: LtCoreAuthDeviceId,
}

impl LtContract for LtCoreDeleteMembersDeviceReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<()>;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Delete)
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/members/{}/devices/{}",
            self.member_id, self.device_id
        )))
    }
}

impl AuthReq for LtCoreDeleteMembersDeviceReq {}
