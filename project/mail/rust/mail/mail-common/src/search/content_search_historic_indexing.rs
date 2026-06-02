//! Session-scoped historic content-search indexing (composition root).
//!
//! `mail-common` owns the trait, the per-session service slot, and the
//! [`ContentSearchHistoricIndexingProvider`] hook on [`MailContext`] that
//! downstream crates (e.g. `mail-historic-ephemeral-load` via `mail-uniffi`)
//! pass into [`MailContext::new`]. Without a provider the service defaults
//! to [`NoopContentSearchHistoricIndexing`] so test
//! harnesses and perf experiments that do not drive historic indexing can
//! still construct a [`MailUserContext`].
//!
//! The walker loops themselves live in
//! [`crate::historic_mailbox_walker`] and are content-search-agnostic. This
//! trait is the per-session lifecycle seam (start/cancel/etc.) above the
//! walker, not the walker itself.

use std::sync::Arc;

use async_trait::async_trait;

#[cfg(feature = "foundation_search")]
use crate::MailContextError;
use crate::{MailContextResult, MailUserContext};

pub use mail_search::ContentSearchStartOutcome;

/// Per-session historic indexing orchestrator (multi-batch All Mail pass).
#[async_trait]
pub trait ContentSearchHistoricIndexing: Send + Sync {
    /// Idempotent start of the historic indexing loop.
    async fn start(
        &self,
        ctx: Arc<MailUserContext>,
    ) -> MailContextResult<ContentSearchStartOutcome>;

    /// Persist the user's enable preference; disabling cancels any in-flight run.
    async fn set_enabled(&self, ctx: Arc<MailUserContext>, enabled: bool) -> MailContextResult<()>;

    /// Cancel any in-flight run; optionally wait and wipe local index data.
    async fn cancel_indexing(
        &self,
        ctx: Arc<MailUserContext>,
        clear_data: bool,
    ) -> MailContextResult<()>;

    /// Wipe locally-persisted content-search artifacts without cancelling.
    async fn clear_local_data(&self, ctx: Arc<MailUserContext>) -> MailContextResult<()>;

    /// Best-effort stop when the owning [`MailUserContext`] is torn down.
    fn cancel_on_teardown(&self) {}
}

/// Factory closure that produces a fresh historic-indexing driver per
/// [`MailUserContext`].
///
/// The orchestrator carries per-session state (in-process slot, cancel
/// token, join handle), so each session must own its own instance — hence a
/// factory rather than a shared `Arc<dyn _>`.
pub type ContentSearchHistoricIndexingProvider =
    Arc<dyn Fn() -> Arc<dyn ContentSearchHistoricIndexing + Send + Sync> + Send + Sync>;

/// Per-session handle stored in [`MailUserContext::services`].
pub struct ContentSearchHistoricIndexingService {
    inner: Arc<dyn ContentSearchHistoricIndexing + Send + Sync>,
}

impl ContentSearchHistoricIndexingService {
    /// Build a service backed by the [`NoopContentSearchHistoricIndexing`]
    /// driver. Used for test harnesses and any [`MailContext`] constructed
    /// without a provider at [`crate::MailContext::new`].
    #[cfg(feature = "foundation_search")]
    #[must_use]
    pub fn noop() -> Self {
        Self::with_driver(Arc::new(NoopContentSearchHistoricIndexing))
    }

    /// Build a service backed by the supplied driver.
    #[must_use]
    pub fn with_driver(driver: Arc<dyn ContentSearchHistoricIndexing + Send + Sync>) -> Self {
        Self { inner: driver }
    }

    pub async fn start(
        &self,
        ctx: Arc<MailUserContext>,
    ) -> MailContextResult<ContentSearchStartOutcome> {
        self.inner.start(ctx).await
    }

    pub async fn set_enabled(
        &self,
        ctx: Arc<MailUserContext>,
        enabled: bool,
    ) -> MailContextResult<()> {
        self.inner.set_enabled(ctx, enabled).await
    }

    pub async fn cancel_indexing(
        &self,
        ctx: Arc<MailUserContext>,
        clear_data: bool,
    ) -> MailContextResult<()> {
        self.inner.cancel_indexing(ctx, clear_data).await
    }

    pub async fn clear_local_data(&self, ctx: Arc<MailUserContext>) -> MailContextResult<()> {
        self.inner.clear_local_data(ctx).await
    }

    fn cancel_on_teardown(&self) {
        self.inner.cancel_on_teardown();
    }

    pub(crate) fn driver(&self) -> Arc<dyn ContentSearchHistoricIndexing + Send + Sync> {
        Arc::clone(&self.inner)
    }
}

impl Drop for ContentSearchHistoricIndexingService {
    fn drop(&mut self) {
        self.cancel_on_teardown();
    }
}

/// Default used when no provider was registered on [`crate::MailContext`]
/// (tests and harnesses that do not drive historic indexing).
#[cfg(feature = "foundation_search")]
pub struct NoopContentSearchHistoricIndexing;

#[cfg(feature = "foundation_search")]
#[async_trait]
impl ContentSearchHistoricIndexing for NoopContentSearchHistoricIndexing {
    async fn start(
        &self,
        _ctx: Arc<MailUserContext>,
    ) -> MailContextResult<ContentSearchStartOutcome> {
        Ok(ContentSearchStartOutcome::NoWork)
    }

    async fn set_enabled(&self, ctx: Arc<MailUserContext>, enabled: bool) -> MailContextResult<()> {
        ctx.search_service()
            .set_indexing_enabled(enabled)
            .await
            .map_err(|e| MailContextError::Other(e.into_inner()))
    }

    async fn cancel_indexing(
        &self,
        ctx: Arc<MailUserContext>,
        clear_data: bool,
    ) -> MailContextResult<()> {
        if clear_data {
            let task_service = ctx.core_context().task_service().task_service_arc();
            ctx.search_service()
                .clear_index_tables(task_service)
                .await
                .map_err(|e| MailContextError::Other(e.into_inner()))?;
        }
        Ok(())
    }

    async fn clear_local_data(&self, ctx: Arc<MailUserContext>) -> MailContextResult<()> {
        let task_service = ctx.core_context().task_service().task_service_arc();
        ctx.search_service()
            .clear_index_tables(task_service)
            .await
            .map_err(|e| MailContextError::Other(e.into_inner()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "foundation_search")]
    #[test]
    fn noop_service_constructs_without_panicking() {
        let _service = ContentSearchHistoricIndexingService::noop();
    }
}
