use std::borrow::Cow;

use crate::{AuthReq, LatticeContract, LatticeError, auth::LtAuthEventId, core::LtCoreEvents};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetEventsIdReq {
    pub event_id: LtAuthEventId,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetEventsLatestReq;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetEventsRes {
    #[cfg_attr(feature = "serde", serde(rename = "EventID"))]
    pub event_id: LtAuthEventId,
    #[cfg_attr(feature = "serde", serde(flatten, default))]
    pub events: LtCoreEvents,
}

impl LatticeContract for LtCoreGetEventsIdReq {
    type Response = LtCoreGetEventsRes;
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/core/v6/events/{}", self.event_id.0)))
    }
}

impl AuthReq for LtCoreGetEventsIdReq {}

impl LatticeContract for LtCoreGetEventsLatestReq {
    type Response = LtCoreGetEventsRes;
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v6/events/latest"))
    }
}

impl AuthReq for LtCoreGetEventsLatestReq {}
