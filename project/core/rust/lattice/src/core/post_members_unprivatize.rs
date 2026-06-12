use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

use super::ids::LtCoreMemberEncId;
use super::unpriv_types::{LtCoreUnprivInvitationData, LtCoreUnprivInvitationSignature};

/// `POST /core/v4/members/{id}/unprivatize` — admin requests unprivatization for an SSO member.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCorePostMembersUnprivatizeReq {
    pub member_id: LtCoreMemberEncId,
    pub body: LtCorePostMembersUnprivatizeBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePostMembersUnprivatizeBody {
    pub invitation_data: LtCoreUnprivInvitationData,
    pub invitation_signature: LtCoreUnprivInvitationSignature,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LtCorePostMembersUnprivatizeRes {}

impl LtContract for LtCorePostMembersUnprivatizeReq {
    type Response = LtSlimAPIJSON<LtCorePostMembersUnprivatizeRes>;
    type Body<'a> = LtSlimAPIJSON<&'a LtCorePostMembersUnprivatizeBody>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(&self.body)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/members/{}/unprivatize",
            self.member_id
        )))
    }
}

impl AuthReq for LtCorePostMembersUnprivatizeReq {}
