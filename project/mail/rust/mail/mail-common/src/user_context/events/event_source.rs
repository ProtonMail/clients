use core_event_loop::v6::EventSource;
use mail_api::services::proton::prelude::MailEventV5;
use mail_core_common::event_loop::v6::CoreEventCache;

pub struct MailEventSourceV5;

impl EventSource for MailEventSourceV5 {
    type Event = MailEventV5;
    type Cache = CoreEventCache;

    fn name() -> &'static str {
        "mail-event-source"
    }
}
