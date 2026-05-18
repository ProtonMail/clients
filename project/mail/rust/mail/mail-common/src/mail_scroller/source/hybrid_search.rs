use super::MailPaginatorJoinHandle;
use crate::datatypes::SearchOptions;
use crate::mail_scroller::{CategoryView, MailScrollerSource};
use crate::models::{MailBusyLabel, Message, MessageCounter, MessageLabel, SearchScrollData};
use crate::{AppError, MailContextError, MailUserContext};
use mail_core_api::services::proton::LabelId;
use mail_core_common::datatypes::LocalLabelId;
use mail_core_common::models::{Label, ModelIdExtension};
use mail_stash::orm::Model;
use mail_stash::stash::{StashError, Tether};
use std::cmp;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};

use super::SearchScrollerSource;

/// State of the hybrid search source. Encodes valid combinations of mode and cursor.
///
/// The type system prevents invalid states (e.g. has_local_results=true but last=None)
/// by making them unrepresentable.
#[derive(Debug)]
enum HybridSourceState {
    /// Before first sync_next; no cursor yet.
    Uninitialized,

    /// Local-only mode: offline or explicitly local. Local results from on-device index.
    /// `resolve_last` re-queries DB (same as Hybrid). Never spawns remote.
    LocalOnly {
        /// Cursor for pagination; re-fetched from DB when needed.
        last: Option<SearchScrollData>,
    },

    /// Hybrid mode: local results first, remote appended in background.
    /// `resolve_last` always re-queries DB to pick up remote append (invariant).
    /// `sync_next` re-fetches last before each pagination step (invariant).
    Hybrid {
        /// Cursor for pagination; re-fetched from DB when needed.
        last: Option<SearchScrollData>,
    },

    /// Remote-only mode: no local results.
    /// `resolve_last` uses cached last; no concurrent append.
    RemoteOnly {
        /// Cached cursor; no re-fetch needed.
        last: Option<SearchScrollData>,
    },
}

impl HybridSourceState {
    /// True iff we have a cursor (last) and thus can serve visible_elements / seen_count.
    fn has_cursor(&self) -> bool {
        matches!(
            self,
            HybridSourceState::LocalOnly { last: Some(_) }
                | HybridSourceState::Hybrid { last: Some(_) }
                | HybridSourceState::RemoteOnly { last: Some(_) }
        )
    }

    /// Returns last for display/seen/has_more.
    /// Hybrid and LocalOnly: re-query DB. RemoteOnly: use cached.
    async fn resolve_last(&self, tether: &Tether) -> Result<Option<SearchScrollData>, StashError> {
        match self {
            HybridSourceState::Uninitialized => Ok(None),
            HybridSourceState::LocalOnly { .. } | HybridSourceState::Hybrid { .. } => {
                // fresh from DB — picks up remote rows (Hybrid) or local pagination (LocalOnly)
                SearchScrollData::last(tether).await
            }
            HybridSourceState::RemoteOnly { last } => Ok(last.clone()),
        }
    }
}

/// A search source that combines local (on-device index) and remote (Proton API) results.
///
/// On initialize, it first populates SearchScrollData from the local index for instant results,
/// then spawns remote search in the background to supplement. If local search yields nothing
/// or is unavailable, it falls back to remote-only behaviour identical to `SearchScrollerSource`.
#[derive(Debug)]
pub struct HybridSearchScrollerSource {
    local_label_id: LocalLabelId,
    options: SearchOptions,
    page_size: usize,
    state: HybridSourceState,
    /// True after first sync_next has run.
    /// Distinguishes "first page" (visible_elements) from "next page" (fetch_more).
    first_sync_done: bool,
    total: Arc<Mutex<u64>>,
    invalidate: Option<flume::Sender<()>>,
    category_view: CategoryView,
}

impl HybridSearchScrollerSource {
    pub fn new(local_label_id: LocalLabelId, options: SearchOptions, page_size: usize) -> Self {
        Self {
            local_label_id,
            options,
            page_size,
            state: HybridSourceState::Uninitialized,
            first_sync_done: false,
            total: Arc::new(Mutex::new(0)),
            invalidate: None,
            category_view: CategoryView::default(),
        }
    }

    async fn initialize_impl(
        &mut self,
        ctx: &MailUserContext,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        let mut tether = ctx.user_stash().connection();

        tether
            .write_tx(async |tx| SearchScrollData::clear_all_search_data(tx).await)
            .await?;

        let Some(remote_label_id) =
            Label::local_id_counterpart(self.local_label_id, &tether).await?
        else {
            return Err(AppError::LabelDoesNotHaveRemoteId(self.local_label_id).into());
        };

        // Try local search first for instant results
        if let Some(keywords) = &self.options.keywords
            && !keywords.trim().is_empty()
        {
            match Self::try_local_search(ctx, keywords, &mut tether).await {
                Ok(true) => {
                    let last = SearchScrollData::last(&tether).await?;

                    // Populate total with local count for hybrid/local-only with local hits
                    if let Some(ref last) = last {
                        let count = last.visible_element_count(&tether).await?;
                        *self.total.lock().await = count;
                    }

                    let is_offline = ctx.network_monitor_service().is_os_offline();

                    if is_offline {
                        self.state = HybridSourceState::LocalOnly { last };
                        info!(
                            "Local search populated results, offline — operating on local index only"
                        );
                        return Ok(None);
                    }

                    self.state = HybridSourceState::Hybrid { last };
                    info!("Local search populated results, spawning remote sync in background");

                    return self
                        .spawn_first_page_and_refresh(
                            ctx,
                            remote_label_id,
                            self.options.clone(),
                            self.page_size,
                        )
                        .await;
                }
                Ok(false) => {
                    debug!("Local search returned no results, falling back to remote");
                }
                Err(e) => {
                    warn!("Local search failed, falling back to remote: {}", e);
                }
            }
        }

        // Fall back to remote-only search
        self.state = HybridSourceState::RemoteOnly { last: None };

        SearchScrollerSource::spawn_first_page(
            ctx,
            self.total.clone(),
            remote_label_id,
            self.options.clone(),
            self.page_size,
            false, // remote-only: use API total
        )
        .await
    }

    #[cfg(feature = "foundation_search")]
    async fn try_local_search(
        ctx: &MailUserContext,
        query: &str,
        tether: &mut Tether,
    ) -> Result<bool, MailContextError> {
        use super::LocalSearchScrollerSource;

        LocalSearchScrollerSource::perform_local_search(ctx, query, tether).await?;
        let has_results = SearchScrollData::last(tether).await?.is_some();
        Ok(has_results)
    }

    #[cfg(not(feature = "foundation_search"))]
    async fn try_local_search(
        _ctx: &MailUserContext,
        _query: &str,
        _tether: &mut Tether,
    ) -> Result<bool, MailContextError> {
        Ok(false)
    }

    /// Spawns first-page sync in the background and triggers mail scroller refresh on completion.
    /// Uses `self.invalidate` to force reload when remote results arrive.
    async fn spawn_first_page_and_refresh(
        &self,
        ctx: &MailUserContext,
        remote_label_id: LabelId,
        search: SearchOptions,
        page_size: usize,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        let task = SearchScrollerSource::spawn_first_page(
            ctx,
            self.total.clone(),
            remote_label_id,
            search,
            page_size,
            true, // hybrid: use deduped count from SearchScrollData
        )
        .await?;
        if let Some(handle) = task {
            let invalidate = self.invalidate.clone();
            ctx.spawn(async move {
                if let Err(e) = handle.await {
                    warn!("Background remote search task failed: {:?}", e);
                }
                if let Some(inv) = invalidate {
                    let _ = inv.send_async(()).await;
                }
            });
        }
        Ok(None)
    }

    async fn total(&self, tether: &Tether) -> Result<u64, StashError> {
        let total = *self.total.lock().await;
        let last = self.state.resolve_last(tether).await?;

        Ok(match &last {
            Some(last) if last.has_more(tether).await? => cmp::max(
                total,
                last.visible_element_count(tether).await? + self.page_size as u64,
            ),
            Some(last) => cmp::max(total, last.visible_element_count(tether).await?),
            None => total,
        })
    }

    #[cfg(debug_assertions)]
    async fn assert_invariants(&self, tether: &Tether) -> Result<(), String> {
        let last = self
            .state
            .resolve_last(tether)
            .await
            .map_err(|e| e.to_string())?;
        let Some(ref last) = last else {
            return Ok(());
        };

        // invariant: seen <= total
        let seen = last
            .visible_element_count(tether)
            .await
            .map_err(|e| e.to_string())?;
        let total = *self.total.lock().await;
        if seen > total {
            return Err(format!(
                "invariant violated: seen ({seen}) > total ({total})"
            ));
        }

        Ok(())
    }
}

impl MailScrollerSource for HybridSearchScrollerSource {
    type Item = Message;

    #[instrument(skip_all)]
    async fn initialize(
        &mut self,
        ctx: &MailUserContext,
        invalidate: flume::Sender<()>,
        category_view: CategoryView,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        self.invalidate = Some(invalidate);
        self.category_view = category_view;
        self.initialize_impl(ctx).await
    }

    async fn visible_elements(
        &self,
        ctx: &MailUserContext,
    ) -> Result<Vec<Self::Item>, MailContextError> {
        let tether = ctx.user_stash().connection();

        if !self.state.has_cursor() {
            Ok(vec![])
        } else {
            let last = self.state.resolve_last(&tether).await?;
            if let Some(ref last) = last {
                Ok(last.visible_elements(&tether).await?)
            } else {
                Ok(vec![])
            }
        }
    }

    async fn seen_count(&self, ctx: &MailUserContext) -> Result<u64, MailContextError> {
        let tether = ctx.user_stash().connection();

        if !self.state.has_cursor() {
            Ok(0)
        } else {
            let last = self.state.resolve_last(&tether).await?;
            if let Some(ref last) = last {
                Ok(last.visible_element_count(&tether).await?)
            } else {
                Ok(0)
            }
        }
    }

    async fn synced_total(&self, ctx: &MailUserContext) -> Result<u64, MailContextError> {
        self.seen_count(ctx).await
    }

    async fn all_total(&self, ctx: &MailUserContext) -> Result<u64, MailContextError> {
        let tether = ctx.user_stash().connection();
        Ok(self.total(&tether).await?)
    }

    async fn has_more(&self, ctx: &MailUserContext) -> Result<bool, MailContextError> {
        let tether = ctx.user_stash().connection();
        let last = self.state.resolve_last(&tether).await?;
        let has_more = match &last {
            Some(last) => last.has_more(&tether).await?,
            None => false,
        };
        Ok(has_more)
    }

    #[instrument(skip(ctx))]
    async fn sync_next(
        &mut self,
        ctx: &MailUserContext,
    ) -> Result<(Vec<Self::Item>, MailPaginatorJoinHandle), MailContextError> {
        let tether = ctx.user_stash().connection();

        // Exhaustive match on state instead of boolean branches.
        match &mut self.state {
            HybridSourceState::Uninitialized => {
                let last = SearchScrollData::last(&tether).await?;
                self.state = HybridSourceState::RemoteOnly { last };
            }
            HybridSourceState::LocalOnly { last } | HybridSourceState::Hybrid { last } => {
                // invariant: re-fetch last before each pagination step
                *last = SearchScrollData::last(&tether).await?;
            }
            HybridSourceState::RemoteOnly { last } => {
                if last.is_none() {
                    *last = SearchScrollData::last(&tether).await?;
                }
            }
        }

        let items = match &mut self.state {
            HybridSourceState::LocalOnly { last }
            | HybridSourceState::Hybrid { last }
            | HybridSourceState::RemoteOnly { last } => {
                if let Some(cursor) = last {
                    if self.first_sync_done {
                        cursor.fetch_more(self.page_size, &tether).await?
                    } else {
                        self.first_sync_done = true;
                        cursor.visible_elements(&tether).await?
                    }
                } else {
                    vec![]
                }
            }
            HybridSourceState::Uninitialized => vec![],
        };

        // LocalOnly: never spawn remote. Hybrid/RemoteOnly: spawn when we have items.
        let task = if items.is_empty() || matches!(&self.state, HybridSourceState::LocalOnly { .. })
        {
            None
        } else {
            let Some(remote_label_id) =
                Label::local_id_counterpart(self.local_label_id, &tether).await?
            else {
                return Err(AppError::LabelDoesNotHaveRemoteId(self.local_label_id).into());
            };

            SearchScrollerSource::spawn_background_sync(
                ctx,
                remote_label_id,
                self.options.clone(),
                self.page_size,
            )
            .await?
        };

        #[cfg(debug_assertions)]
        if let Err(e) = self.assert_invariants(&tether).await {
            warn!("Hybrid scroller invariant check: {}", e);
        }

        Ok((items, task))
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
        self.state = HybridSourceState::Uninitialized;
        self.first_sync_done = false;
        self.initialize_impl(ctx).await
    }

    fn watched_tables(&self) -> Vec<String> {
        vec![
            Message::table_name().to_owned(),
            MessageLabel::table_name().to_owned(),
            MessageCounter::table_name().to_owned(),
            MailBusyLabel::table_name().to_owned(),
            SearchScrollData::table_name().to_owned(),
        ]
    }

    async fn change_state(
        &mut self,
        ctx: &MailUserContext,
        _unread: Option<crate::datatypes::ReadFilter>,
        label: Option<LocalLabelId>,
        keywords: Option<SearchOptions>,
        category_view: Option<CategoryView>,
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

        if let Some(v) = category_view {
            self.category_view = v;
        }

        self.state = HybridSourceState::Uninitialized;
        self.first_sync_done = false;
        self.initialize_impl(ctx).await
    }

    fn category_view(&self) -> &CategoryView {
        &self.category_view
    }
}
