use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, auth::LtAuthEventId,
    core::LtCoreEvents,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCoreGetEventsIdReq {
    pub event_id: LtAuthEventId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetEventsIdRes {
    #[serde(rename = "EventID")]
    pub event_id: LtAuthEventId,
    #[serde(flatten, default)]
    pub events: LtCoreEvents,
}

impl LtContract for LtCoreGetEventsIdReq {
    type Response = LtSlimAPIJSON<LtCoreGetEventsIdRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/core/v6/events/{}", self.event_id.0)))
    }
}

impl AuthReq for LtCoreGetEventsIdReq {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCoreGetEventsIdRawReq {
    pub event_id: LtAuthEventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtCoreGetEventsIdRawRes(pub String);

impl Serialize for LtCoreGetEventsIdRawRes {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match serde_json::from_str::<serde_json::Value>(&self.0) {
            Ok(value) => value.serialize(ser),
            Err(e) => Err(serde::ser::Error::custom(e)),
        }
    }
}

impl<'de> Deserialize<'de> for LtCoreGetEventsIdRawRes {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match serde_json::Value::deserialize(de) {
            Ok(value) => Ok(Self(value.to_string())),
            Err(e) => Err(serde::de::Error::custom(e)),
        }
    }
}

impl LtContract for LtCoreGetEventsIdRawReq {
    type Response = LtSlimAPIJSON<LtCoreGetEventsIdRawRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/core/v6/events/{}", self.event_id.0)))
    }
}

impl AuthReq for LtCoreGetEventsIdRawReq {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCoreGetEventsLatestReq;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetEventsLatestRes {
    #[serde(rename = "EventID")]
    pub event_id: LtAuthEventId,
}

impl LtContract for LtCoreGetEventsLatestReq {
    type Response = LtSlimAPIJSON<LtCoreGetEventsLatestRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v6/events/latest"))
    }
}

impl AuthReq for LtCoreGetEventsLatestReq {}
