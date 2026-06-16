use std::ops::Deref;

use mail_event_service::EventService;

#[derive(Default)]
pub struct UserEventService {
    event_service: EventService,
}

impl UserEventService {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Deref for UserEventService {
    type Target = EventService;
    fn deref(&self) -> &Self::Target {
        &self.event_service
    }
}
