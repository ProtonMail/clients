use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

use super::ids::LtCoreMemberEncId;
use super::unpriv_types::{LtCoreUnprivInvitationData, LtCoreUnprivInvitationSignature};

/// `POST /core/v4/members/{id}/unprivatize` — admin requests unprivatization for an SSO member.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCorePostMembersUnprivatizeReq {
    pub member_id: LtCoreMemberEncId,
    pub body: LtCorePostMembersUnprivatizeBody,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePostMembersUnprivatizeBody {
    pub invitation_data: LtCoreUnprivInvitationData,
    pub invitation_signature: LtCoreUnprivInvitationSignature,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
