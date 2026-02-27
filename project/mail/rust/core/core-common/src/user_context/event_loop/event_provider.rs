use crate::CoreEventLoopContext;
use crate::services::event_loop_service::EventManagerContext;
use async_trait::async_trait;
use core_event_loop::RawEvent;
use core_event_loop::provider::{EventProvider, EventProviderError, EventProviderResult};
use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::ProtonCore;

#[derive(Debug, thiserror::Error)]
pub enum CoreEventProviderError {
    #[error(transparent)]
    Api(#[from] ApiServiceError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl EventProviderError for CoreEventProviderError {
    fn is_network_failure(&self) -> bool {
        match self {
            CoreEventProviderError::Api(e) => e.is_network_failure(),
            CoreEventProviderError::Other(_) => false,
        }
    }
}

#[async_trait]
impl EventProvider<EventManagerContext> for CoreEventLoopContext {
    async fn get_latest_event_id(
        &self,
        _: &EventManagerContext,
    ) -> EventProviderResult<core_event_loop::EventId> {
        async {
            let ctx = self.inner()?;
            Ok::<_, CoreEventProviderError>(
                ctx.session()
                    .get_events_latest()
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
        _: &EventManagerContext,
        event_id: &core_event_loop::EventId,
    ) -> EventProviderResult<RawEvent> {
        async {
            let ctx = self.inner()?;
            let json_string = ctx
                .session()
                .get_event(
                    event_id.clone().into_inner().into(),
                    mail_core_api::services::proton::GetEventOptions::all(),
                )
                .await?;

            Ok::<_, CoreEventProviderError>(RawEvent::from_json(json_string)?)
        }
        .await
        .map_err(|e| -> Box<dyn EventProviderError> { Box::new(e) })
    }
}
