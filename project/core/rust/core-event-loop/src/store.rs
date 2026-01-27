#![allow(clippy::module_name_repetitions)]

use crate::EventId;
use anyhow::anyhow;
use async_trait::async_trait;

/// This trait allows abstraction over how to store and load events. Note that this only stores the
/// event `RemoteId`, you will need to ask the `Provider` for the actual event.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait EventStore<Ctx>: Send + Sync
where
    Ctx: Send + Sync + 'static,
{
    async fn load(&self, ctx: &Ctx) -> anyhow::Result<Option<EventId>>;
    async fn store(&self, ctx: &Ctx, id: EventId) -> anyhow::Result<()>;
}

#[derive(Debug, Default)]
pub struct InMemoryEventStore {
    id: std::sync::RwLock<Option<EventId>>,
}

#[async_trait]
impl<Ctx> EventStore<Ctx> for InMemoryEventStore
where
    Ctx: Send + Sync + 'static,
{
    async fn load(&self, _: &Ctx) -> anyhow::Result<Option<EventId>> {
        let accessor = self.id.read().map_err(|_| anyhow!("lock poison"))?;
        Ok(accessor.clone())
    }

    async fn store(&self, _: &Ctx, id: EventId) -> anyhow::Result<()> {
        let mut accessor = self.id.write().map_err(|_| anyhow!("lock poison"))?;
        *accessor = Some(id);
        Ok(())
    }
}
