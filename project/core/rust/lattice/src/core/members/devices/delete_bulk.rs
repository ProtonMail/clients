//! `DELETE /core/v4/members/{memberId}/devices` — org admin removes **all** auth devices for a member.
//!
//! Source: `Proton\Apps\Account\Controller\Auth\DeleteAuthDevicesAction::deleteAdmin`. Scope: `ORGANIZATION`.

use std::borrow::Cow;

use crate::core::ids::LtCoreMemberEncId;
use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON, Method};

/// No body; `memberId` selects the org member.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreDeleteMembersDevicesReq {
    pub member_id: LtCoreMemberEncId,
}

impl LtContract for LtCoreDeleteMembersDevicesReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<()>;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Delete)
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/members/{}/devices",
            self.member_id
        )))
    }
}

impl AuthReq for LtCoreDeleteMembersDevicesReq {}
