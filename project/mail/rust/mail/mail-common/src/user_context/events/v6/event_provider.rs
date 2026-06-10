use crate::events::v6::MailEventLoopV6Context;
use async_trait::async_trait;
use core_event_loop::{EventProvider, EventProviderError, EventProviderResult, RawEvent};
use mail_api::services::proton::ProtonMail;
use mail_core_api::service::ApiServiceError;
use mail_core_common::services::event_loop_service::EventManagerContext;

#[derive(Debug, thiserror::Error)]
pub enum MailEventProviderError {
    #[error(transparent)]
    Api(#[from] ApiServiceError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl EventProviderError for MailEventProviderError {
    fn is_network_failure(&self) -> bool {
        match self {
            MailEventProviderError::Api(e) => e.is_network_failure(),
            MailEventProviderError::Other(_) => false,
        }
    }

    fn is_retryable(&self) -> bool {
        match self {
            Self::Api(e) => e.is_network_failure() || e.is_server_failure() || e.is_auth_failure(),
            _ => false,
        }
    }
}

#[async_trait]
impl EventProvider<EventManagerContext> for MailEventLoopV6Context {
    async fn get_latest_event_id(
        &self,
        _: &EventManagerContext,
    ) -> EventProviderResult<core_event_loop::EventId> {
        async {
            let ctx = self.inner()?;
            Ok::<_, MailEventProviderError>(
                ctx.session()
                    .get_mail_event_latest_v6()
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
                .get_mail_event_v6(event_id.clone().into_inner().into())
                .await?;

            Ok::<_, MailEventProviderError>(RawEvent::from_json(json_string)?)
        }
        .await
        .map_err(|e| -> Box<dyn EventProviderError> { Box::new(e) })
    }
}
