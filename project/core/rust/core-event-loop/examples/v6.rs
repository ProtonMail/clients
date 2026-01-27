#![allow(unused_variables)]

use core_event_loop::v6::{
    EventSource, EventSourceDependencyList, EventSubscriber, EventSubscriberResult,
};
use core_event_loop::{
    EventId, EventProvider, EventProviderResult, RawEvent, RefreshFlag, store::EventStore,
};
use serde::Deserialize;
struct CoreEventSource;

struct Context {
    //TODO fill me out
}

#[derive(Debug, Deserialize)]
struct CoreEvent {
    //TODO: fill me out
}

impl EventSource for CoreEventSource {
    type Event = CoreEvent;
    type Cache = ();
    fn name() -> &'static str {
        "Core"
    }
}

struct MailEventSource;

#[derive(Debug, Deserialize)]
struct MailEvent {
    //TODO: fill me out
}

impl EventSource for MailEventSource {
    type Event = MailEvent;
    type Cache = ();
    fn name() -> &'static str {
        "Core"
    }
    fn dependencies() -> EventSourceDependencyList {
        EventSourceDependencyList::default().with::<CoreEventSource>()
    }
}

struct DummyEventProvider;

#[async_trait::async_trait]
impl EventProvider<Context> for DummyEventProvider {
    async fn get_latest_event_id(&self, ctx: &Context) -> EventProviderResult<EventId> {
        todo!()
    }
    async fn get_event(&self, ctx: &Context, event_id: &EventId) -> EventProviderResult<RawEvent> {
        todo!()
    }
}

struct DummyEventStore;

struct MailSubscriber;

#[async_trait::async_trait]
impl EventSubscriber<Context, MailEventSource> for MailSubscriber {
    fn name(&self) -> &'static str {
        "mail_subscriber"
    }

    async fn on_event(
        &self,
        ctx: &Context,
        event: &MailEvent,
        cache: &mut (),
    ) -> EventSubscriberResult<()> {
        todo!()
    }
    async fn on_refresh(
        &self,
        ctx: &Context,
        _: RefreshFlag,
        (): &mut (),
    ) -> EventSubscriberResult<()> {
        todo!()
    }
}

#[async_trait::async_trait]
impl EventStore<Context> for DummyEventStore {
    async fn load(&self, ctx: &Context) -> anyhow::Result<Option<EventId>> {
        todo!()
    }
    async fn store(&self, ctx: &Context, id: EventId) -> anyhow::Result<()> {
        todo!()
    }
}

#[tokio::main]
async fn main() {
    use core_event_loop::v6::EventManager;
    let context = Context {};
    let mut manager = EventManager::new();
    manager
        .add::<CoreEventSource>(Box::new(DummyEventProvider), Box::new(DummyEventStore))
        .unwrap();
    manager.initialize_all(&context).await.unwrap();
    let subscriber_id = manager.subscribe(MailSubscriber).unwrap();
    manager.poll(&context).await.unwrap();
    manager.unsubscribe(subscriber_id);
}
