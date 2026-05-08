use std::borrow::Cow;

use crate::CONTACTS_V6;
use lattice::{LtContract, LtNoQueryParams, LtRawBody, LtSlimAPIJSON};
use mail_api_event_types::{EventId, GetEventsLatestResponse};

pub struct GetContactEventLatestRequest;

impl LtContract for GetContactEventLatestRequest {
    type Response = LtSlimAPIJSON<GetEventsLatestResponse>;
    type Body<'b> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<std::borrow::Cow<'a, str>, lattice::LatticeError> {
        Ok(Cow::Owned(format!("{CONTACTS_V6}/events/latest")))
    }
}

#[cfg(feature = "mocks")]
impl GetContactEventLatestRequest {
    pub fn mock() -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("GET")).and(path(format!("api{CONTACTS_V6}/events/latest")))
    }
}

pub struct GetContactEvent {
    pub id: EventId,
}

impl LtContract for GetContactEvent {
    type Response = LtRawBody;
    type Body<'b> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, lattice::LatticeError> {
        Ok(Cow::Owned(format!("{CONTACTS_V6}/events/{}", self.id)))
    }
}

#[cfg(feature = "mocks")]
impl GetContactEvent {
    pub fn mock(id: EventId) -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("GET")).and(path(format!("api{CONTACTS_V6}/events/{id}")))
    }
}
