use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON, Method};

use super::LtCoreMemberEncId;

/// Request to trigger SAML authentication for a member
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCorePostMembersSamlReq {
    /// The member ID (encrypted ID)
    #[cfg_attr(feature = "serde", serde(skip))]
    pub member_id: LtCoreMemberEncId,
}

/// Response from the member SAML endpoint
/// Contains only the success code (1000)
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCorePostMembersSamlRes {}

impl LtContract for LtCorePostMembersSamlReq {
    type Response = LtSlimAPIJSON<LtCorePostMembersSamlRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/members/{}/saml",
            self.member_id
        )))
    }
}

impl AuthReq for LtCorePostMembersSamlReq {}
