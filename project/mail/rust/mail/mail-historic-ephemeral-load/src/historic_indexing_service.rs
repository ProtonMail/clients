//! [`ContentSearchHistoricIndexing`] implementation for the production
//! [`ContentSearchIndexingOrchestrator`] plus the public provider helper
//! consumed by the composition root.
//!
//! The composition root (`mail-uniffi` for production, the perf harness for
//! `mail-search-perf`) passes [`historic_indexing_provider`] into
//! [`mail_common::MailContext::new`]. `mail-common` then builds a fresh
//! [`ContentSearchIndexingOrchestrator`] for each [`MailUserContext`] it
//! constructs.

use std::sync::Arc;

use async_trait::async_trait;
use mail_common::search::{
    ContentSearchHistoricIndexing, ContentSearchHistoricIndexingProvider, ContentSearchStartOutcome,
};
use mail_common::{MailContextError, MailContextResult, MailUserContext};

use crate::orchestrator::ContentSearchIndexingOrchestrator;

#[async_trait]
impl ContentSearchHistoricIndexing for ContentSearchIndexingOrchestrator {
    async fn start(
        &self,
        ctx: Arc<MailUserContext>,
    ) -> MailContextResult<ContentSearchStartOutcome> {
        ContentSearchIndexingOrchestrator::start(self, ctx)
            .await
            .map_err(MailContextError::from)
    }

    async fn set_enabled(&self, ctx: Arc<MailUserContext>, enabled: bool) -> MailContextResult<()> {
        ContentSearchIndexingOrchestrator::set_enabled(self, ctx, enabled)
            .await
            .map_err(MailContextError::from)
    }

    async fn cancel_indexing(
        &self,
        ctx: Arc<MailUserContext>,
        clear_data: bool,
    ) -> MailContextResult<()> {
        ContentSearchIndexingOrchestrator::cancel_indexing(self, ctx, clear_data)
            .await
            .map_err(MailContextError::from)
    }

    async fn clear_local_data(&self, ctx: Arc<MailUserContext>) -> MailContextResult<()> {
        ContentSearchIndexingOrchestrator::clear_local_data(self, ctx)
            .await
            .map_err(MailContextError::from)
    }

    fn cancel_on_teardown(&self) {
        ContentSearchIndexingOrchestrator::cancel(self);
    }
}

/// Build the per-session historic-indexing provider for the production
/// [`ContentSearchIndexingOrchestrator`].
///
/// Pass the returned closure to [`mail_common::MailContext::new`] from the composition
/// root; `mail-common` will invoke it each time it constructs a
/// [`MailUserContext`].
#[must_use]
pub fn historic_indexing_provider() -> ContentSearchHistoricIndexingProvider {
    Arc::new(|| Arc::new(ContentSearchIndexingOrchestrator::new()))
}
