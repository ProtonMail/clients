use super::MailPaginatorJoinHandle;
use crate::{
    MailContextError, MailUserContext,
    datatypes::SearchOptions,
    mail_scroller::MailScrollerSource,
    models::{MailBusyLabel, Message, MessageCounter, MessageLabel, SearchScrollData},
};
use mail_core_common::datatypes::LocalLabelId;
use mail_core_common::models::ModelExtension;
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::{Bond, StashError, Tether};
use tracing::{debug, error, info, instrument, warn};

#[derive(Debug)]
pub struct LocalSearchScrollerSource {
    local_label_id: LocalLabelId,
    options: SearchOptions,
    page_size: usize,
    initialized: bool,
    /// This is the last synced item (highest display_order). Used consistently by
    /// visible_elements, seen_count, and has_more. It is safe on deletes: queries only need
    /// display_order — soft-deleted messages are excluded by predicate
    /// `messages.deleted = 0`, and CASCADE-deleted rows don't affect the cached display_order value.
    last: Option<SearchScrollData>,
    invalidate: Option<flume::Sender<()>>,
}

impl LocalSearchScrollerSource {
    pub fn new(local_label_id: LocalLabelId, options: SearchOptions, page_size: usize) -> Self {
        Self {
            local_label_id,
            options,
            page_size,
            initialized: false,
            last: None,
            invalidate: None,
        }
    }

    async fn initialize_impl(
        &mut self,
        ctx: &MailUserContext,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        let mut tether = ctx.user_stash().connection().await?;

        tether
            .tx(async |tx| SearchScrollData::clear_all_search_data(tx).await)
            .await?;

        let Some(keywords) = &self.options.keywords else {
            debug!("No keywords provided for local search");
            return Ok(None);
        };

        if keywords.trim().is_empty() {
            debug!("Empty keywords provided for local search");
            return Ok(None);
        }

        if let Err(e) = Self::perform_local_search(ctx, keywords, &mut tether).await {
            error!("Local search failed: {}", e);
            return Err(e);
        }

        let last_scroll_data = SearchScrollData::last(&tether).await?;
        self.last = last_scroll_data;

        Ok(None)
    }

    /// Perform local search and populate SearchScrollData with results.
    /// Callable from hybrid_search to avoid duplicating this logic.
    pub(crate) async fn perform_local_search(
        ctx: &MailUserContext,
        query: &str,
        tether: &mut Tether,
    ) -> Result<(), MailContextError> {
        use crate::search::search_local_with_keywords;

        let search_service = ctx.search_service();

        // Get local search results
        let local_results = search_local_with_keywords(search_service, tether, query)
            .await
            .map_err(|e| {
                tracing::error!("Local search failed: {:#?}", e);
                MailContextError::Other(anyhow::anyhow!("Local search failed: {:#?}", e))
            })?;

        if local_results.is_empty() {
            let stats = search_service.get_stats().await;
            tracing::warn!(
                "Search index stats: {} documents total, is_writing: {}",
                stats.documents_total,
                stats.is_writing
            );
            return Ok(());
        }

        debug!(
            "Saving {} local search results to SearchScrollData",
            local_results.len()
        );

        let saved_count = tether
            .quiet_tx::<_, usize, StashError>(async |tx: &Bond<'_, UserDb>| {
                let mut display_order = 0_u64;
                let mut saved = 0_usize;

                for result in local_results {
                    // Load the message to ensure it exists
                    let Some(message) = Message::find_by_id(result.local_message_id, tx).await?
                    else {
                        debug!("Message {} not found, skipping", result.local_message_id);
                        // Message was deleted - skip
                        continue;
                    };

                    SearchScrollData::builder()
                        .local_message_id(message.id())
                        .display_order(display_order)
                        .build()
                        .with_save(tx)
                        .await?;

                    // Always persist so the client has a row and can get the search query. Body
                    // highlighting is term-based (find query terms in HTML), not position-based,
                    // because we index plain text but display HTML — positions would be wrong for body.
                    match serde_json::to_string(&result.matches) {
                        Ok(json) => {
                            use crate::models::SearchHighlighting;
                            SearchHighlighting::builder()
                                .local_message_id(message.id())
                                .highlighting_positions(json)
                                .build()
                                .with_save(tx)
                                .await?;
                        }
                        Err(e) => {
                            warn!("Failed to serialize highlighting positions: {}", e);
                        }
                    }

                    debug!(
                        "Saved message {} to SearchScrollData with display_order {}",
                        message.id(),
                        display_order
                    );
                    saved = saved.saturating_add(1);
                    display_order = display_order.saturating_add(1);
                }

                Ok(saved)
            })
            .await
            .map_err(|e| {
                MailContextError::Other(anyhow::anyhow!(
                    "Failed to save local search results: {}",
                    e
                ))
            })?;

        info!("Saved {} messages to SearchScrollData", saved_count);
        Ok(())
    }
}

impl MailScrollerSource for LocalSearchScrollerSource {
    type Item = Message;

    #[instrument(skip_all)]
    async fn initialize(
        &mut self,
        ctx: &MailUserContext,
        invalidate: flume::Sender<()>,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        self.invalidate = Some(invalidate);
        self.initialize_impl(ctx).await
    }

    async fn visible_elements(
        &self,
        ctx: &MailUserContext,
    ) -> Result<Vec<Self::Item>, MailContextError> {
        let tether = ctx.user_stash().connection().await?;

        if !self.initialized {
            Ok(vec![])
        } else if let Some(ref last) = self.last {
            Ok(last.visible_elements(&tether).await?)
        } else {
            Ok(vec![])
        }
    }

    async fn seen_count(&self, ctx: &MailUserContext) -> Result<u64, MailContextError> {
        let tether = ctx.user_stash().connection().await?;

        if !self.initialized {
            Ok(0)
        } else if let Some(ref last) = self.last {
            Ok(last.visible_element_count(&tether).await?)
        } else {
            Ok(0)
        }
    }

    async fn synced_total(&self, ctx: &MailUserContext) -> Result<u64, MailContextError> {
        self.seen_count(ctx).await
    }

    async fn all_total(&self, ctx: &MailUserContext) -> Result<u64, MailContextError> {
        // For local search, the total is the number of results we have
        self.seen_count(ctx).await
    }

    async fn has_more(&self, ctx: &MailUserContext) -> Result<bool, MailContextError> {
        let tether = ctx.user_stash().connection().await?;
        let has_more = match &self.last {
            Some(last) if self.initialized => last.has_more(&tether).await?,
            _ => false,
        };
        Ok(has_more)
    }

    #[instrument(skip(ctx))]
    async fn sync_next(
        &mut self,
        ctx: &MailUserContext,
    ) -> Result<(Vec<Self::Item>, MailPaginatorJoinHandle), MailContextError> {
        let tether = ctx.user_stash().connection().await?;

        self.last = SearchScrollData::last(&tether).await?;

        if let Some(ref mut last) = self.last {
            let items = if self.initialized {
                last.fetch_more(self.page_size, &tether).await?
            } else {
                self.initialized = true;
                last.visible_elements(&tether).await?
            };

            Ok((items, None))
        } else {
            Ok((vec![], None))
        }
    }

    async fn sync_new(
        &mut self,
        _ctx: &MailUserContext,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        Ok(None)
    }

    async fn clear(
        &mut self,
        ctx: &MailUserContext,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        self.initialize_impl(ctx).await
    }

    fn watched_tables(&self) -> Vec<String> {
        vec![
            Message::table_name().to_owned(),
            MessageLabel::table_name().to_owned(),
            MessageCounter::table_name().to_owned(),
            MailBusyLabel::table_name().to_owned(),
        ]
    }

    async fn change_state(
        &mut self,
        ctx: &MailUserContext,
        _unread: Option<crate::datatypes::ReadFilter>,
        label: Option<LocalLabelId>,
        keywords: Option<SearchOptions>,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        if let Some(label) = label {
            info!(
                "Changing label from {current:?} to {label:?}",
                current = self.local_label_id
            );
            self.local_label_id = label;
        }

        if let Some(keywords) = keywords {
            info!("Changing search parameters");
            self.options = keywords;
        }

        self.initialized = false;
        self.last = None;
        self.initialize_impl(ctx).await
    }
}
