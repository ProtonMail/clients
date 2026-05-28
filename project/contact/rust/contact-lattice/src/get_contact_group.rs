use std::borrow::Cow;
use std::collections::HashMap;

use lattice::{
    LatticeError, LtContract, LtNoQueryParams, LtSerdeQueryParams, LtSlimAPIJSON, Method,
};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{CONTACT_GROUP_LABEL_TYPE, CORE_V4, ContactGroup, ContactGroupId};

#[derive(Debug)]
pub struct GetContactGroupsRequest;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct GetContactGroupsResponse {
    #[serde(deserialize_with = "deserialize_contact_group")]
    pub labels: Vec<ContactGroup>,
}

#[derive(Serialize)]
pub struct GetContactGroupsQueryParams {
    #[serde(rename = "Type")]
    pub label_type: u32,
}

impl Default for GetContactGroupsQueryParams {
    fn default() -> Self {
        Self {
            label_type: CONTACT_GROUP_LABEL_TYPE,
        }
    }
}

impl LtContract for GetContactGroupsRequest {
    type Response = LtSlimAPIJSON<GetContactGroupsResponse>;

    type Body<'b> = LtSlimAPIJSON<()>;

    type Query<'q> = LtSerdeQueryParams<GetContactGroupsQueryParams>;

    fn path<'a>(&'a self) -> Result<std::borrow::Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("{CORE_V4}/labels")))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        Some(LtSerdeQueryParams(GetContactGroupsQueryParams::default()))
    }
}

#[cfg(feature = "mocks")]
impl GetContactGroupsRequest {
    pub fn mock() -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("GET"))
            .and(path(format!("api{CORE_V4}/labels")))
            .and(query_param_contains(
                "Type",
                CONTACT_GROUP_LABEL_TYPE.to_string(),
            ))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GetContactGroupsByIdsRequest {
    pub ids: Vec<ContactGroupId>,
}

impl LtContract for GetContactGroupsByIdsRequest {
    type Response = LtSlimAPIJSON<GetContactGroupsResponse>;

    type Body<'b> = LtSlimAPIJSON<&'b Self>;

    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("{CORE_V4}/labels/by-ids")))
    }

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }
}

#[cfg(feature = "mocks")]
impl GetContactGroupsByIdsRequest {
    pub fn mock(&self) -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("GET")).and(path(format!("api{CORE_V4}/labels")))
    }
}

fn deserialize_contact_group<'de, D>(deserializer: D) -> Result<Vec<ContactGroup>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    pub enum MapOrList {
        Map(HashMap<String, ContactGroup>),
        List(Vec<ContactGroup>),
    }

    impl MapOrList {
        pub fn into_vec(self) -> Vec<ContactGroup> {
            match self {
                MapOrList::Map(map) => map.into_values().collect(),
                MapOrList::List(list) => list,
            }
        }
    }

    MapOrList::deserialize(deserializer).map(MapOrList::into_vec)
}
