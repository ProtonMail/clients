use crate::event_loop::v6::CoreEventCache;
use core_event_loop::v6::EventSource;
use proton_core_api::services::proton::CoreEvent;

pub struct CoreEventSource;

impl EventSource for CoreEventSource {
    type Event = CoreEvent;
    type Cache = CoreEventCache;

    fn name() -> &'static str {
        "core"
    }
}
