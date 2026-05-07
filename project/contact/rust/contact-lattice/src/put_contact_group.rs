use std::borrow::Cow;

use lattice::{LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};
use serde::{Deserialize, Serialize};

use crate::{CORE_V4, ContactGroup, ContactGroupId};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PutContactGroupRequest {
    #[serde(skip)]
    pub id: ContactGroupId,
    pub color: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct PutContactGroupResponse {
    #[serde(rename = "Label")]
    pub contact_group: ContactGroup,
}

impl LtContract for PutContactGroupRequest {
    type Response = LtSlimAPIJSON<PutContactGroupResponse>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("{CORE_V4}/labels/{}", self.id)))
    }

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(self)))
    }
}

#[cfg(feature = "mocks")]
impl PutContactGroupRequest {
    pub fn mock(&self) -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("PUT"))
            .and(path(format!("api{CORE_V4}/{}", self.id)))
            .and(body_json(self))
    }
}
