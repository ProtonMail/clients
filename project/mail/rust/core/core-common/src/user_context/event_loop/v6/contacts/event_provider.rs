use crate::event_loop::v6::ContactEventLoopV6Context;
use crate::{
    event_loop::event_provider::CoreEventProviderError,
    services::event_loop_service::EventManagerContext,
};
use async_trait::async_trait;
use contacts_api::ContactApi;
use core_event_loop::{EventProvider, EventProviderError, EventProviderResult, RawEvent};

#[async_trait]
impl EventProvider<EventManagerContext> for ContactEventLoopV6Context {
    async fn get_latest_event_id(
        &self,
        ctx: &EventManagerContext,
    ) -> EventProviderResult<core_event_loop::EventId> {
        async {
            Ok::<_, CoreEventProviderError>(
                ctx.session()
                    .get_contact_event_latest_v6()
                    .await?
                    .event_id
                    .into_inner()
                    .into(),
            )
        }
        .await
        .map_err(|e| -> Box<dyn EventProviderError> { Box::new(e) })
    }

    async fn get_event(
        &self,
        ctx: &EventManagerContext,
        event_id: &core_event_loop::EventId,
    ) -> EventProviderResult<RawEvent> {
        async {
            let json_string = ctx
                .session()
                .get_contact_event_v6(event_id.clone().into_inner().into())
                .await?;

            Ok::<_, CoreEventProviderError>(RawEvent::from_json(json_string)?)
        }
        .await
        .map_err(|e| -> Box<dyn EventProviderError> { Box::new(e) })
    }
}
