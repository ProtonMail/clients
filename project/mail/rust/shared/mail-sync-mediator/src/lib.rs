//! A simple server resource sync mediator.
//!
//! Allows various tasks to fetch the same resource from the server at
//! the same time using only one network request. The mediator also ensures
//! if this resource has already been fetched, it is not refetched.
//!
//! Fetched content remains in memory up to a configurable amount of time
//! to handle a corner case where storing the data in the store happens
//! right after the load check.
//!

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use parking_lot::Mutex;

#[allow(async_fn_in_trait)]
pub trait SyncResource: 'static {
    type Item: Send + Clone + Sync + 'static;
    type Key: Send + Clone + Eq + Hash + std::fmt::Debug + 'static;
    type FetchError: std::error::Error + Send + Sync;
    type StoreError: std::error::Error + Send + Sync;
    type Context: Send + 'static;

    fn fetch(
        ctx: &Self::Context,
        key: &Self::Key,
    ) -> impl Future<Output = Result<Self::Item, Self::FetchError>> + Send;

    fn store(
        ctx: &Self::Context,
        key: &Self::Key,
        item: &mut Self::Item,
    ) -> impl Future<Output = Result<(), Self::StoreError>> + Send;

    /// Returning `None` here means the resource does not exist locally
    /// and should be retrieved from the server.
    fn load(
        ctx: &Self::Context,
        key: &Self::Key,
    ) -> impl Future<Output = Result<Option<Self::Item>, Self::StoreError>> + Send;
}

#[derive(Debug, thiserror::Error)]
pub enum SyncMediatorError<R: SyncResource> {
    #[error(transparent)]
    Fetch(R::FetchError),
    #[error(transparent)]
    Store(R::StoreError),
}

pub struct SyncMediator<R: SyncResource> {
    in_flight: Arc<Mutex<HashMap<R::Key, SyncState<R>>>>,
    cache_ttl: Duration,
}

enum SyncState<R: SyncResource> {
    InFlight(InFlightFetchFuture<R>),
    Cached(R::Item, Instant),
}

impl<R: SyncResource> Clone for SyncState<R> {
    fn clone(&self) -> Self {
        match self {
            Self::InFlight(v) => Self::InFlight(v.clone()),
            Self::Cached(v, t) => Self::Cached(v.clone(), *t),
        }
    }
}

impl<R> SyncMediator<R>
where
    R: SyncResource,
{
    /// Creates a sync mediator with a default cache ttl of 1 minute.
    ///
    /// See also [`with_cache_ttl`] for custom values.
    pub fn new() -> Self {
        Self::with_cache_ttl(DEFAULT_CACHED_ITEM_TTL)
    }

    pub fn with_cache_ttl(cache_ttl: Duration) -> Self {
        Self {
            in_flight: Arc::new(Mutex::new(HashMap::new())),
            cache_ttl,
        }
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn sync(
        &self,
        ctx: R::Context,
        key: &R::Key,
    ) -> Result<R::Item, Arc<SyncMediatorError<R>>> {
        // Strip out old cached data.
        {
            let mut in_flight = self.in_flight.lock();
            in_flight.retain(|_, v| match v {
                SyncState::InFlight(_) => true,
                SyncState::Cached(_, instant) => instant.elapsed() < self.cache_ttl,
            });
        }
        // check if present
        if let Some(item) = R::load(&ctx, key)
            .await
            .map_err(|e| Arc::new(SyncMediatorError::Store(e)))?
        {
            tracing::debug!("Available locally, skipping fetch");
            return Ok(item);
        }

        match self.get_or_create_future(ctx, key) {
            SyncState::InFlight(shared) => shared.await,
            SyncState::Cached(item, _) => Ok(item),
        }
    }

    // Separate function to prevent capture of values into the wrapped future from tracing
    // instrument
    fn get_or_create_future(&self, ctx: R::Context, key: &R::Key) -> SyncState<R> {
        // if not present schedule fetch
        let mut in_flight = self.in_flight.lock();

        let future = match in_flight.entry(key.clone()) {
            Entry::Occupied(occupied) => {
                tracing::debug!("Request in flight, joining...");
                (*occupied.get()).clone()
            }
            Entry::Vacant(vacant) => {
                tracing::debug!("No in flight request, initiating fetch");
                let in_flight_cloned = self.in_flight.clone();
                let key = key.clone();
                let future = async move {
                    let mut item = R::fetch(&ctx, &key)
                        .await
                        .map_err(|e| Arc::new(SyncMediatorError::Fetch(e)))?;
                    R::store(&ctx, &key, &mut item)
                        .await
                        .map_err(|e| Arc::new(SyncMediatorError::Store(e)))?;
                    // clear in flight progress
                    {
                        let mut in_flight = in_flight_cloned.lock();
                        in_flight.insert(key, SyncState::Cached(item.clone(), Instant::now()));
                        drop(in_flight);
                    }
                    tracing::debug!("Item synced");
                    Ok::<_, Arc<SyncMediatorError<R>>>(item)
                }
                .boxed()
                .shared();
                let r = SyncState::InFlight(future);
                vacant.insert(r.clone());
                r
            }
        };

        drop(in_flight);
        future
    }
}

impl<R: SyncResource> Default for SyncMediator<R> {
    fn default() -> Self {
        Self::new()
    }
}

const DEFAULT_CACHED_ITEM_TTL: Duration = std::time::Duration::from_secs(60);

#[allow(type_alias_bounds, reason = "Private type +enforced in SyncMediator")]
type InFlightFetchFuture<R: SyncResource> =
    Shared<BoxFuture<'static, Result<R::Item, Arc<SyncMediatorError<R>>>>>;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use mockall::Sequence;
    use mockall::predicate::eq;

    #[derive(Debug, thiserror::Error)]
    #[error("Fetch failed")]
    struct TestFetchError;

    #[derive(Debug, thiserror::Error)]
    #[error("Store failed")]
    struct TestStoreError;

    mockall::mock! {
        TestResourceContext {
            fn load(&self,key:u32) -> Result<Option<()>,TestStoreError>;
            fn store(&self,key:u32) -> Result<(), TestStoreError>;
            fn fetch(&self,key:u32) -> Result<(), TestFetchError>;
        }
    }

    #[derive(Debug)]
    struct TestResource;

    impl SyncResource for TestResource {
        type Item = ();
        type Key = u32;
        type FetchError = TestFetchError;
        type StoreError = TestStoreError;
        type Context = Arc<MockTestResourceContext>;

        async fn fetch(
            ctx: &Self::Context,
            key: &Self::Key,
        ) -> Result<Self::Item, Self::FetchError> {
            tokio::time::sleep(Duration::from_millis(200)).await;
            ctx.fetch(*key)
        }

        async fn store(
            ctx: &Self::Context,
            key: &Self::Key,
            _: &mut Self::Item,
        ) -> Result<(), Self::StoreError> {
            ctx.store(*key)
        }

        async fn load(
            ctx: &Self::Context,
            key: &Self::Key,
        ) -> Result<Option<Self::Item>, Self::StoreError> {
            ctx.load(*key)
        }
    }

    type Mediator = SyncMediator<TestResource>;

    #[tokio::test]
    async fn resource_fetched_if_not_in_store() {
        let mut mock_ctx = MockTestResourceContext::new();
        let key = 10_u32;

        let mut sequence = Sequence::new();
        mock_ctx
            .expect_load()
            .with(eq(key))
            .returning(|_| Ok(None))
            .times(1)
            .in_sequence(&mut sequence);
        mock_ctx
            .expect_fetch()
            .with(eq(key))
            .returning(|_| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);
        mock_ctx
            .expect_store()
            .with(eq(key))
            .returning(|_| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);
        mock_ctx
            .expect_load()
            .with(eq(key))
            .returning(|_| Ok(Some(())))
            .times(1)
            .in_sequence(&mut sequence);

        let ctx = Arc::new(mock_ctx);

        let mediator = Mediator::new();
        mediator.sync(ctx.clone(), &key).await.unwrap();
        mediator.sync(ctx, &key).await.unwrap();
    }

    #[tokio::test]
    async fn resource_only_fetched_once_from_remote() {
        let mut mock_ctx = MockTestResourceContext::new();
        let key = 10_u32;

        mock_ctx.expect_load().with(eq(key)).returning(|_| Ok(None));
        mock_ctx
            .expect_fetch()
            .with(eq(key))
            .returning(|_| Ok(()))
            .times(1);
        mock_ctx
            .expect_store()
            .with(eq(key))
            .returning(|_| Ok(()))
            .times(1);

        let ctx = Arc::new(mock_ctx);

        let mediator = Arc::new(Mediator::new());

        let (ready_tx, mut ready_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let (start_tx, start_rx) = tokio::sync::mpsc::channel::<()>(1);

        let tasks = (0..10)
            .map(|_| {
                let ctx = ctx.clone();
                let mediator = mediator.clone();

                let ready_tx = ready_tx.clone();
                let start_tx = start_tx.clone();

                tokio::spawn(async move {
                    ready_tx.send(()).unwrap();
                    let _ = start_tx.send(()).await;
                    mediator.sync(ctx, &key).await
                })
            })
            .collect::<Vec<_>>();

        for _ in 0..10 {
            tokio::time::timeout(Duration::from_secs(5), ready_rx.recv())
                .await
                .unwrap()
                .unwrap();
        }
        drop(start_rx);

        for task in tasks {
            tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        }
    }

    #[tokio::test]
    async fn resource_remove_after_ttl() {
        let mut mock_ctx = MockTestResourceContext::new();
        let key = 10_u32;

        let mut sequence = Sequence::new();
        mock_ctx
            .expect_load()
            .with(eq(key))
            .returning(|_| Ok(None))
            .times(1)
            .in_sequence(&mut sequence);
        mock_ctx
            .expect_fetch()
            .with(eq(key))
            .returning(|_| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);
        mock_ctx
            .expect_store()
            .with(eq(key))
            .returning(|_| Ok(()))
            .times(1)
            .in_sequence(&mut sequence);

        let ctx = Arc::new(mock_ctx);

        let mediator = Mediator::new();
        mediator.in_flight.lock().insert(
            key,
            SyncState::Cached(
                (),
                Instant::now()
                    .checked_sub(DEFAULT_CACHED_ITEM_TTL * 2)
                    .unwrap(),
            ),
        );
        mediator.sync(ctx.clone(), &key).await.unwrap();
    }
}
