use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, auth::LtAuthEventId,
    core::LtCoreEvents,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetEventsIdReq {
    pub event_id: LtAuthEventId,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetEventsIdRes {
    #[cfg_attr(feature = "serde", serde(rename = "EventID"))]
    pub event_id: LtAuthEventId,
    #[cfg_attr(feature = "serde", serde(flatten, default))]
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetEventsIdRawReq {
    pub event_id: LtAuthEventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtCoreGetEventsIdRawRes(pub String);

#[cfg(feature = "serde")]
impl serde::Serialize for LtCoreGetEventsIdRawRes {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match serde_json::from_str::<serde_json::Value>(&self.0) {
            Ok(value) => value.serialize(ser),
            Err(e) => Err(serde::ser::Error::custom(e)),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for LtCoreGetEventsIdRawRes {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetEventsLatestReq;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetEventsLatestRes {
    #[cfg_attr(feature = "serde", serde(rename = "EventID"))]
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
