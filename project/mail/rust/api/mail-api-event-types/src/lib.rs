//! Shared API event types.

use serde::Deserialize;
#[cfg(feature = "mocks")]
use serde::Serialize;
use serde_repr::Deserialize_repr;
#[cfg(feature = "mocks")]
use serde_repr::Serialize_repr;

mail_proton_ids::declare_proton_id! {
    pub EventId
}

impl From<core_event_loop::EventId> for EventId {
    fn from(event_id: core_event_loop::EventId) -> Self {
        Self::from(event_id.into_inner())
    }
}

/// The action associated with an API event.
#[derive(Clone, Copy, Debug, Deserialize_repr, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize_repr))]
#[repr(u8)]
pub enum Action {
    Delete = 0,
    Create = 1,
    Update = 2,
    UpdateFlags = 3,
}

/// The response for the latest event endpoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct GetEventsLatestResponse {
    /// TODO: Document this field.
    #[serde(rename = "EventID")]
    pub event_id: EventId,
}
