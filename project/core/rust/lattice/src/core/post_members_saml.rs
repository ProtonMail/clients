use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

use super::LtCoreMemberEncId;

/// Request to trigger SAML authentication for a member
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCorePostMembersSamlReq {
    /// The member ID (encrypted ID)
    #[serde(skip)]
    pub member_id: LtCoreMemberEncId,
}

/// Response from the member SAML endpoint
/// Contains only the success code (1000)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCorePostMembersSamlRes {}

impl LtContract for LtCorePostMembersSamlReq {
    type Response = LtSlimAPIJSON<LtCorePostMembersSamlRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

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
