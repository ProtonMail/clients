use crate::events::v6::MailEventLoopV6Context;
use async_trait::async_trait;
use core_event_loop::store::EventStore;
use mail_core_common::event_loop::event_store;
use mail_core_common::event_loop::event_store::MAIL_EVENT_TYPE_ID;
use mail_core_common::services::event_loop_service::EventManagerContext;

#[async_trait]
impl EventStore<EventManagerContext> for MailEventLoopV6Context {
    async fn load(
        &self,
        ctx: &EventManagerContext,
    ) -> anyhow::Result<Option<core_event_loop::EventId>> {
        event_store::load_event_id(ctx, MAIL_EVENT_TYPE_ID).await
    }

    async fn store(
        &self,
        ctx: &EventManagerContext,
        id: core_event_loop::EventId,
    ) -> anyhow::Result<()> {
        event_store::store_event_id(ctx, MAIL_EVENT_TYPE_ID, id).await
    }
}
