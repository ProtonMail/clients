use std::borrow::Cow;

use lattice::{LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};
use serde::{Deserialize, Serialize};

use crate::{CONTACT_GROUP_LABEL_TYPE, CORE_V4, ContactGroup};

pub struct PostContactGroupRequest {
    pub color: String,
    pub name: String,
}

// Label type is required, but we only create one type of label with this request.
// Hid this detail in a private type
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PostContactGroupRequestPrivate<'a> {
    #[serde(rename = "Type")]
    label_type: u32,
    color: &'a str,
    name: &'a str,
}

impl<'a> PostContactGroupRequestPrivate<'a> {
    fn from_request(value: &'a PostContactGroupRequest) -> PostContactGroupRequestPrivate<'a> {
        Self {
            label_type: CONTACT_GROUP_LABEL_TYPE,
            color: value.color.as_str(),
            name: value.name.as_str(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct PostContactGroupResponse {
    #[serde(rename = "Label")]
    pub contact_group: ContactGroup,
}

impl LtContract for PostContactGroupRequest {
    type Response = LtSlimAPIJSON<PostContactGroupResponse>;
    type Body<'b> = LtSlimAPIJSON<PostContactGroupRequestPrivate<'b>>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("{CORE_V4}/labels")))
    }

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(
            PostContactGroupRequestPrivate::from_request(self),
        )))
    }
}

#[cfg(feature = "mocks")]
impl PostContactGroupRequest {
    pub fn mock(&self) -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("POST"))
            .and(path(format!("api{CORE_V4}")))
            .and(body_json(PostContactGroupRequestPrivate::from_request(
                self,
            )))
    }
}
