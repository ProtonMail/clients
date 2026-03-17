use crate::event_loop::event_store::CORE_EVENT_TYPE_ID;
use crate::event_loop::v6::CoreEventLoopV6Context;
use crate::services::event_loop_service::EventManagerContext;
use crate::user_context::event_loop::event_store;
use async_trait::async_trait;
use core_event_loop::store::EventStore;

#[async_trait]
impl EventStore<EventManagerContext> for CoreEventLoopV6Context {
    async fn load(
        &self,
        ctx: &EventManagerContext,
    ) -> anyhow::Result<Option<core_event_loop::EventId>> {
        event_store::load_event_id(ctx, CORE_EVENT_TYPE_ID).await
    }

    async fn store(
        &self,
        ctx: &EventManagerContext,
        id: core_event_loop::EventId,
    ) -> anyhow::Result<()> {
        event_store::store_event_id(ctx, CORE_EVENT_TYPE_ID, id).await
    }
}
