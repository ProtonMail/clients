use crate::events::{ContactEventLoopV6Context, ContactEventSessionContext};
use async_trait::async_trait;
use contacts_api::ContactApi;
use core_event_loop::{EventProvider, EventProviderError, EventProviderResult, RawEvent};
use mail_core_api::service::ApiServiceError;

#[derive(Debug, thiserror::Error)]
pub enum ContactEventProviderError {
    #[error(transparent)]
    Api(#[from] ApiServiceError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl EventProviderError for ContactEventProviderError {
    fn is_network_failure(&self) -> bool {
        match self {
            Self::Api(e) => e.is_network_failure(),
            Self::Other(_) => false,
        }
    }
}

#[async_trait]
impl<T: ContactEventSessionContext> EventProvider<T> for ContactEventLoopV6Context {
    async fn get_latest_event_id(&self, ctx: &T) -> EventProviderResult<core_event_loop::EventId> {
        async {
            Ok::<_, ContactEventProviderError>(
                ctx.get_contact_api()
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
        ctx: &T,
        event_id: &core_event_loop::EventId,
    ) -> EventProviderResult<RawEvent> {
        async {
            let json_string = ctx
                .get_contact_api()
                .get_contact_event_v6(event_id.clone().into_inner().into())
                .await?;

            Ok::<_, ContactEventProviderError>(RawEvent::from_json(json_string)?)
        }
        .await
        .map_err(|e| -> Box<dyn EventProviderError> { Box::new(e) })
    }
}
