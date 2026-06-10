#![allow(clippy::module_name_repetitions)]

use async_trait::async_trait;
// avoid namespace conflicts
use crate::{EventId, RawEvent};

pub trait EventProviderError: std::error::Error + Send + Sync + 'static {
    fn is_network_failure(&self) -> bool;
    fn is_retryable(&self) -> bool {
        self.is_network_failure()
    }
}

pub type EventProviderResult<T> = Result<T, Box<dyn EventProviderError>>;

/// This trait allows abstraction over how to request the next event from the API.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait EventProvider<Ctx>: Send + Sync
where
    Ctx: Send + Sync + 'static,
{
    async fn get_latest_event_id(&self, ctx: &Ctx) -> EventProviderResult<EventId>;
    async fn get_event(&self, ctx: &Ctx, event_id: &EventId) -> EventProviderResult<RawEvent>;
}
