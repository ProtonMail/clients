#![allow(clippy::result_large_err)]

mod action_queue;
mod attachment_cache;
mod builder;
pub mod events;
mod images;
mod initialization;

use crate::actions::PREFETCH_ROLLBACK_ACTION_GROUP;
use crate::actions::draft::{SEND_ACTION_GROUP, SHARE_EXT_ACTION_GROUP};
use crate::db::online_migrations;
use crate::draft::attachments::DraftStagingAreaCleaner;
#[cfg(feature = "events-v6")]
use crate::events::v6;
use crate::models::Message;
use crate::prefetch::{Prefetch, PrefetchJob, PrefetchService};
use crate::rsvp::RsvpService;
use crate::search::{
    ContentSearchHistoricIndexingService, ContentSearchStartOutcome, MailSearchService,
};
use crate::upsell_eligibility::UpsellEligibilityService;
use crate::user_context::events::event_source::MailEventSourceV5;
use crate::{ImageLoader, MailContext, MailContextError, MailContextResult, TrackerService};
use anyhow::anyhow;
use attachment_cache::AttachmentCacheState;
use builder::MailUserContextBuilder;
use core_event_loop::EventLoopError;
use core_event_loop::v6::{EventSubscriber, EventSubscriberId};
use events::event_subscriber::MailEventV5Subscriber;
use initialization::InitializationMediator;
use mail_account_api::password::PasswordFlow;
use mail_action_queue::action::ActionGroup;
use mail_action_queue::queue::{
    OnlineStatusWaiterBuilder, Queue, QueueAutoExecutor, QueueAutoExecutorPool,
};
use mail_core_api::connection_status::ConnectionStatus;
use mail_core_api::crypto_clock;
use mail_core_api::services::proton::{PingApi, PrivateEmailRef, SessionId, UserId};
use mail_core_api::session::Session;
#[cfg(not(feature = "events-v6"))]
use mail_core_common::CoreEventLoopContext;
use mail_core_common::actions::event_poll::EVENT_POLL_ACTION_GROUP;
use mail_core_common::datatypes::AccountDetails;
use mail_core_common::event_loop::EventPollMode;
use mail_core_common::models::{User, UserSettings};
use mail_core_common::services::crypto_key_service::CryptoKeyService;
use mail_core_common::services::crypto_key_service::mail_core_key_manager::{
    PublicAddressKeyApiFetchPolicy, PublicAddressKeyContactFetchPolicy,
};
use mail_core_common::services::event_loop_service::EventManagerContext;
use mail_core_common::services::{
    EventLoopService, EventPollConfigService, NetworkMonitorService, UserIssueReporterService,
    UserMetricService,
};
use mail_core_common::{Context as CoreContext, Origin, UserContext};
use mail_crypto_inbox::keys::{ComposerPreference, SendPreferences};
use mail_crypto_inbox::proton_crypto::CryptoClockProvider;
use mail_crypto_inbox::proton_crypto::crypto::PGPProviderSync;
use mail_issue_reporter_service::{IssueLevel, issue_report_keys_from_error};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::{Stash, Tether};
use mail_task_service::Spawner;
use parking_lot::Mutex;
use proton_crypto_account::keys::{CryptoMailSettings, VerificationPreferences};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, error, instrument};

const DEFAULT_SEND_QUEUE_POOL_SIZE: usize = 4;
const DEFAULT_DEFAULT_QUEUE_POOL_SIZE: usize = 1;

const DEFAULT_PREFETCH_ROLLBACK_QUEUE_POOL_SIZE: usize = 1;
const DEFAULT_SHARE_EXT_QUEUE_POOL_SIZE: usize = 2;

const DEFAULT_PREFETCH_BOUND: usize = 10;

/// App origin only
pub struct DefaultQueueExecutor {
    pub default: QueueAutoExecutorPool<UserDb>,
    pub prefetch_rollback: QueueAutoExecutorPool<UserDb>,
}

impl DefaultQueueExecutor {
    #[must_use]
    pub fn prefetch_rollback_worker_count(&self) -> usize {
        self.prefetch_rollback.worker_count()
    }

    pub fn pause(&self) {
        self.default.pause();
        self.prefetch_rollback.pause();
    }

    pub fn pause_prefetch_rollback(&self) {
        self.prefetch_rollback.pause();
    }

    pub fn resume(&self) {
        self.default.resume();
        self.prefetch_rollback.resume();
    }

    pub fn resume_prefetch_rollback(&self) {
        self.prefetch_rollback.resume();
    }

    pub fn terminate(&self) {
        self.default.terminate();
        self.prefetch_rollback.terminate();
    }
}

/// Used by both App and ShareExt origins
pub struct SendQueueExecutorPool {
    pub pool: QueueAutoExecutorPool<UserDb>,
}

impl SendQueueExecutorPool {
    pub fn pause(&self) {
        self.pool.pause();
    }

    pub fn resume(&self) {
        self.pool.resume();
    }

    pub fn terminate(&self) {
        self.pool.terminate();
    }
}

/// Event Poll executor only
pub struct EventPollQueueExecutor {
    pub executor: QueueAutoExecutor<UserDb>,
}

impl EventPollQueueExecutor {
    pub fn pause(&self) {
        self.executor.pause();
    }

    pub fn resume(&self) {
        self.executor.resume();
    }

    pub fn terminate(&self) {
        self.executor.terminate();
    }
}

pub struct QueuesService {
    weak: Weak<MailUserContext>,
}

impl QueuesService {
    pub fn new(weak: Weak<MailUserContext>) -> Self {
        Self { weak }
    }

    #[tracing::instrument(skip_all)]
    pub fn pause(&self) {
        let Some(ctx) = self.weak.upgrade() else {
            tracing::error!("Could not upgrade weak ctx reference");
            return;
        };
        if let Some(service) = ctx.get_service_opt::<DefaultQueueExecutor>() {
            service.pause();
        };
        if let Some(service) = ctx.get_service_opt::<EventPollQueueExecutor>() {
            service.pause();
        }
        ctx.get_service::<SendQueueExecutorPool>().pause();
    }

    #[tracing::instrument(skip_all)]
    pub fn resume(&self) {
        let Some(ctx) = self.weak.upgrade() else {
            tracing::error!("Could not upgrade weak ctx reference");
            return;
        };
        if let Some(service) = ctx.get_service_opt::<DefaultQueueExecutor>() {
            service.resume();
        };
        if let Some(service) = ctx.get_service_opt::<EventPollQueueExecutor>() {
            service.resume();
        }
        ctx.get_service::<SendQueueExecutorPool>().resume();
    }

    #[tracing::instrument(skip_all)]
    pub fn terminate(&self) {
        let Some(ctx) = self.weak.upgrade() else {
            tracing::error!("Could not upgrade weak ctx reference");
            return;
        };
        if let Some(service) = ctx.get_service_opt::<DefaultQueueExecutor>() {
            service.terminate();
        };
        if let Some(service) = ctx.get_service_opt::<EventPollQueueExecutor>() {
            service.terminate();
        }
        ctx.get_service::<SendQueueExecutorPool>().terminate();
    }
}

#[derive(Default)]
struct EventSubscriberList {
    #[cfg(feature = "events-v6")]
    subscribers: Mutex<Vec<EventSubscriberId<v6::MailEventSourceV6>>>,
    #[cfg(not(feature = "events-v6"))]
    subscribers: Mutex<Vec<EventSubscriberId<MailEventSourceV5>>>,
}

impl EventSubscriberList {
    pub async fn register(&self, ctx: &MailUserContext) -> Result<(), MailContextError> {
        let event_service = ctx.user_context().get_service::<EventLoopService>();
        let event_poll = event_service.event_poll();

        let mut subscriber_list = Vec::new();

        #[cfg(not(feature = "events-v6"))]
        {
            use crate::user_context::events::event_subscribers_compat::*;
            let even_ctx: CoreEventLoopContext = Arc::downgrade(&ctx.user_context).into();
            match event_poll
                .add::<MailEventSourceV5>(Box::new(even_ctx.clone()), Box::new(even_ctx))
                .await
            {
                // Due to current context management, it's possible this can be registered
                // more than once.
                Ok(()) | Err(EventLoopError::DuplicateEventSource(_)) => {}
                Err(e) => return Err(e.into()),
            }
            let core_subscriber = event_poll
                .subscribe(MailEventV5SubscriberCompat(
                    ctx.user_context.event_subscriber(),
                ))
                .await?
                .ok_or_else(|| {
                    MailContextError::Other(anyhow!("Failed to register core subscriber"))
                })?;
            let account_subscriber = event_poll
                .subscribe(MailEventV5SubscriberCompat(
                    ctx.user_context.account_event_subscriber(),
                ))
                .await?
                .ok_or_else(|| {
                    MailContextError::Other(anyhow!("Failed to register account subscriber"))
                })?;
            subscriber_list.push(core_subscriber);
            subscriber_list.push(account_subscriber);
        }

        #[cfg(feature = "events-v6")]
        {
            let event_ctx = v6::MailEventLoopV6Context::from(ctx.this.clone());
            match event_poll
                .add::<v6::MailEventSourceV6>(Box::new(event_ctx.clone()), Box::new(event_ctx))
                .await
            {
                Ok(()) => {}
                Err(EventLoopError::DuplicateEventSource(_)) => {
                    tracing::debug!("Previous mail event source still registered, removing...");
                    // Due to current context management, it's possible this can be registered
                    // more than once. We should cleanup the previous on and re-register
                    event_poll.remove::<v6::MailEventSourceV6>().await?;
                    let event_ctx = v6::MailEventLoopV6Context::from(ctx.this.clone());
                    event_poll
                        .add::<v6::MailEventSourceV6>(
                            Box::new(event_ctx.clone()),
                            Box::new(event_ctx),
                        )
                        .await?;
                }
                Err(e) => return Err(e.into()),
            }

            let mail_subscriber = event_poll
                .subscribe(v6::MailEventV6Subscriber::from(ctx.this.clone()))
                .await?
                .ok_or_else(|| {
                    MailContextError::Other(anyhow!("Failed to register mail v6 subscriber"))
                })?;

            subscriber_list.push(mail_subscriber);
        }

        #[cfg(not(feature = "events-v6"))]
        {
            let mail_subscriber = ctx.event_subscriber();
            let mail_subscriber =
                event_poll
                    .subscribe(mail_subscriber)
                    .await?
                    .ok_or_else(|| {
                        MailContextError::Other(anyhow!("Failed to register mail subscriber"))
                    })?;

            subscriber_list.push(mail_subscriber);
        }

        let mut subscribers = self.subscribers.lock();
        *subscribers = subscriber_list;
        Ok(())
    }

    fn clear_subscribers(&self, ctx: &MailUserContext) {
        let mut subscribers = self.subscribers.lock();
        let subscriber_ids = std::mem::take(&mut *subscribers);
        drop(subscribers);

        let core_ctx = ctx.user_context.clone();
        ctx.spawn(async move {
            let event_service = core_ctx.event_loop_service();
            let event_poll = event_service.event_poll();

            // The astute reader may wonder why we don't remove the mail event sroucre here. If
            // we do we risk a race conditon where the source may be removed after we pass the
            // exists check during the `MailUserContext` creation, leaving us in state of limbo.
            // The registration fo the event source will perform this check. Removing subscribers
            // is still okay since they do nothing if the event source no longer exists or
            // the subscriber handle can't be found.
            for id in subscriber_ids {
                // It's safe to ignore errors here since the only failure possible
                // for unsubscribe is that the actor is dead.
                let _ = event_poll.unsubscribe(id).await;
            }
        });
    }
}

pub struct MailUserContext {
    this: Weak<Self>,
    mail_context: Arc<MailContext>,
    user_context: Arc<UserContext>,
    cancellation_token: CancellationToken,
    services: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl MailUserContext {
    #[instrument(name = "NewMailUserContext", skip_all, fields(mode, user_id=%user_context.user_id()
    ))]
    pub(crate) async fn new(
        mail_context: Arc<MailContext>,
        user_context: Arc<UserContext>,
    ) -> MailContextResult<Arc<Self>> {
        tracing::info!("Creating MailUserContext");
        let user_context_cloned = user_context.clone();

        async {
            let span =
                tracing::debug_span!(parent: None, "qac", user_id = %user_context.user_id().short_id());

        let origin = mail_context.core_context().origin();

            let cancellation_token = user_context_cloned.create_child_cancellation_token();

            let mut builder =
                MailUserContextBuilder::new()
                    .with_service(AttachmentCacheState::new())
                    .with_service(EventSubscriberList::default())
                    .with_cyclic_service(ImageLoader::new)
                    .with_cyclic_service(TrackerService::new)
                    .with_cyclic_service(UpsellEligibilityService::new);

            // Initialize Foundation Search service with Stash connection pool
            // Extract TaskService from the context to ensure proper lifecycle management.
            // The TaskService is extracted from BackgroundAwareTaskService which wraps it.
            if let Some(provider) = mail_context.historic_indexing_provider()
            {
                tracing::info!("Enabling Content Search");
                // Get Arc<TaskService> from BackgroundAwareTaskService
                let task_service = mail_context.core_context().task_service().task_service_arc();

                let search_service = MailSearchService::new(
                    user_context.mail_stash().clone(),
                    task_service,
                )
                .await
                .map_err(|e| {
                    MailContextError::Other(anyhow::anyhow!(
                        "Failed to initialize Foundation Search service: {}",
                        e
                    ))
                })?;
                builder = builder.with_service(search_service);

                // Build the per-session historic indexing service from the
                // provider supplied at `MailContext::new` by the composition
                // root (`mail-uniffi`, perf harness, etc.). Tests and
                // harnesses that pass `None` transparently fall back to the
                // no-op driver.
                let historic_service =
                        crate::search::ContentSearchHistoricIndexingService::with_driver(
                            provider());
                builder = builder.with_service(historic_service);
            } else {
                tracing::info!("Content Search not Enabled");
            }

            builder = match origin {
                Origin::App => {
                    builder
                        .with_service(SendQueueExecutorPool {
                            pool: QueueAutoExecutorPool::new(
                                user_context.queue(),
                                &SEND_ACTION_GROUP,
                                NonZeroUsize::new(DEFAULT_SEND_QUEUE_POOL_SIZE).unwrap(),
                                mail_context.core_context().as_ref(),
                                true,
                                span.clone(),
                            ),
                        })
                        .with_service({
                            let default = QueueAutoExecutorPool::new(
                                user_context.queue(),
                                &ActionGroup::default(),
                                NonZeroUsize::new(DEFAULT_DEFAULT_QUEUE_POOL_SIZE).unwrap(),
                                mail_context.core_context().as_ref(),
                                true,
                                span.clone(),
                            );
                            let prefetch_rollback = QueueAutoExecutorPool::new(
                                user_context.queue(),
                                &PREFETCH_ROLLBACK_ACTION_GROUP,
                                NonZeroUsize::new(DEFAULT_PREFETCH_ROLLBACK_QUEUE_POOL_SIZE)
                                    .unwrap(),
                                mail_context.core_context().as_ref(),
                                true,
                                span.clone(),
                            );
                            DefaultQueueExecutor {
                                default,
                                prefetch_rollback,
                            }
                        })
                        .with_service(EventPollQueueExecutor {
                            executor: user_context.queue().new_executor_with_group(EVENT_POLL_ACTION_GROUP).into_auto_executor(
                                mail_context.core_context().as_ref().build(),
                                true,
                                span.clone(),
                            ),
                        })
                        .with_cyclic_service(QueuesService::new)
                        .with_service(InitializationMediator::new(
                            mail_context.core_context().task_service().task_service(),
                        ))
                        .with_service(RsvpService::new(user_context.mail_stash()))
                        .with_service(PrefetchService::new())
                }

                Origin::ShareExt => builder
                    .with_service(SendQueueExecutorPool {
                        pool: {
                            if let Err(e) = Queue::delete_all_in_group(
                                user_context.queue(),
                                SHARE_EXT_ACTION_GROUP.clone(),
                            )
                                .await
                            {
                                tracing::warn!("Could not clear share extension queue: {}", e);
                                tracing::warn!("Continuing with existing queue");
                            };
                            QueueAutoExecutorPool::new(
                                user_context.queue(),
                                &SHARE_EXT_ACTION_GROUP,
                                NonZeroUsize::new(DEFAULT_SHARE_EXT_QUEUE_POOL_SIZE).unwrap(),
                                mail_context.core_context().as_ref(),
                                true,
                                span.clone(),
                            )
                        },
                    })
                    .with_cyclic_service(QueuesService::new),
            };

            let this = builder.build(mail_context, user_context, cancellation_token).await?;

            // Catch invalid actions at this stage to interrupt the context creation
            // and avoid infinite error loops.
            if let Err(e) = this.user_context.queue().validate_queued_actions().await {
                return Err(MailContextError::NonProcessableActions(e));
            }

            match origin {
                Origin::App => {
                    DraftStagingAreaCleaner::new().run(&this)?;
                    this.init_expiration_loop();
                    this.init_search_worker();
                    this.register_subscribers().await?;
                    online_migrations::run(&this).await?;

                    let config = this
                        .mail_context()
                        .core_context()
                        .get_service::<EventPollConfigService>();

                    if let EventPollMode::Automatic(interval) = config.mode() {
                        this.init_event_loop_poll(interval)?;
                    }
                }

                Origin::ShareExt => {
                    //
                }
            }

            // There's a race condition between initializing queues and `self` - to
            // avoid it, we start our queues as paused and resume once everything
            // has been initialized, i.e. here:
            this.queues().resume();

            tracing::info!("Creating MailUserContext...Done");
            Ok(this)
        }
            .await
            .inspect_err(|e| {
                if !e.is_network_failure() {
                    user_context_cloned.issue_reporter_service().report(
                        IssueLevel::Critical,
                        "Failed to create new mail user context".into(),
                        issue_report_keys_from_error(e),
                    )
                }
            })
    }

    #[must_use]
    pub fn get_service<T: Any + Send + Sync + 'static>(&self) -> &T {
        self.services
            .get(&TypeId::of::<T>())
            .and_then(|service| service.downcast_ref::<T>())
            .unwrap_or_else(|| {
                panic!(
                    "Required service {} not found in context",
                    std::any::type_name::<T>()
                )
            })
    }

    pub fn get_service_opt<T: Any + Send + Sync + 'static>(&self) -> Option<&T> {
        self.services
            .get(&TypeId::of::<T>())
            .and_then(|service| service.downcast_ref::<T>())
    }

    pub fn has_service<T: Any + Send + Sync + 'static>(&self) -> bool {
        self.services.contains_key(&TypeId::of::<T>())
    }

    pub fn queues(&self) -> &QueuesService {
        self.get_service::<QueuesService>()
    }

    #[must_use]
    pub fn as_arc(&self) -> Arc<Self> {
        self.this.upgrade().expect("Should never fail")
    }

    #[must_use]
    pub fn as_weak(&self) -> Weak<Self> {
        Weak::clone(&self.this)
    }

    #[must_use]
    pub fn origin(&self) -> Origin {
        self.core_context().origin()
    }

    /// Sets a background job where every 60 seconds it deletes all of the messages and conversations
    /// that have an expiration date.
    fn init_expiration_loop(&self) {
        let ctx = self.this.clone();
        self.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                let Some(ctx) = ctx.upgrade() else {
                    return;
                };
                let mut tether = ctx.user_stash().connection();
                if let Err(e) = Message::delete_expired(&mut tether).await {
                    error!("Error in background task deleting expired messages: {e:?}");
                }
                drop(ctx);
                interval.tick().await;
            }
        });
    }

    /// Starts the search index worker that processes pending index intents.
    ///
    /// The worker runs in the background and:
    /// - Polls for pending `SearchIndexIntent` records
    /// - Executes indexing/removal operations via `MailSearchService`
    /// - Handles retries with exponential backoff
    /// - Runs cleanup when the intent queue is empty (idempotent)
    ///
    /// If the worker fails to start, it reports the error to Sentry as a critical issue.
    ///
    /// The worker runs on a dedicated thread named `Search Index Worker` with its own
    /// [`tokio::runtime::Runtime`] (`current_thread` scheduler) so bulk indexing does not share
    /// Tokio worker threads with the rest of the mail event loop (prefetch, sync, etc.).
    /// A root [`tracing::Span`] keeps the task identifiable in Perfetto / tracing captures.
    fn init_search_worker(&self) {
        use crate::search::StashMessageDataProvider;
        use std::thread;

        let Some(search_service) = self.search_service().cloned() else {
            return;
        };
        let data_provider = Arc::new(StashMessageDataProvider::new(self.user_stash().clone()));
        let ctx_weak = self.this.clone();
        let shutdown_token = self.cancellation_token.child_token();
        let span = tracing::info_span!("search_index_worker");

        thread::Builder::new()
            .name("Search Index Worker".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(r) => r,
                    Err(e) => {
                        error!("Failed to create search index worker runtime: {}", e);
                        if let Some(ctx) = ctx_weak.upgrade() {
                            ctx.issue_reporter_service().report(
                                IssueLevel::Critical,
                                format!("Failed to create search index worker runtime: {e}"),
                                issue_report_keys_from_error(&e),
                            );
                        }
                        return;
                    }
                };

                rt.block_on(
                    async move {
                        let (shutdown_handle, worker_shutdown_signal) =
                            crate::search::WorkerShutdownHandle::pair();
                        let shutdown_wait = shutdown_token;
                        tokio::spawn(async move {
                            shutdown_wait.cancelled().await;
                            let _ = shutdown_handle.request_shutdown();
                        });

                        match search_service.create_worker(data_provider).await {
                            Ok(worker) => {
                                worker.run(worker_shutdown_signal).await;
                            }
                            Err(e) => {
                                error!("Failed to create search index worker: {}", e);
                                if let Some(ctx) = ctx_weak.upgrade() {
                                    ctx.issue_reporter_service().report(
                                        IssueLevel::Critical,
                                        format!("Failed to create search index worker: {e}"),
                                        issue_report_keys_from_error(&e),
                                    );
                                }
                            }
                        }
                    }
                    .instrument(span),
                );
            })
            .expect("Failed to spawn Search Index Worker thread");
    }

    pub fn session(&self) -> &Session {
        self.user_context.session()
    }

    pub fn action_queue(&self) -> &Queue<UserDb> {
        self.user_context.queue()
    }

    #[must_use]
    pub fn user_stash(&self) -> &Stash<UserDb> {
        self.user_context.mail_stash()
    }

    pub fn mail_context(&self) -> &MailContext {
        &self.mail_context
    }

    pub fn mail_context_arc(&self) -> &Arc<MailContext> {
        &self.mail_context
    }

    pub fn core_context(&self) -> &Arc<CoreContext> {
        self.mail_context.core_context()
    }

    pub fn user_context(&self) -> &UserContext {
        &self.user_context
    }

    pub fn observability_service(&self) -> &UserMetricService {
        self.user_context.get_service::<UserMetricService>()
    }

    /// Access the Foundation Search service for local email indexing and search
    pub fn search_service(&self) -> Option<&MailSearchService> {
        self.get_service_opt::<MailSearchService>()
    }

    /// Per-session historic content-search indexing orchestrator.
    pub fn content_search_historic_indexing(
        &self,
    ) -> Option<&ContentSearchHistoricIndexingService> {
        self.get_service_opt::<ContentSearchHistoricIndexingService>()
    }

    /// Start the historic content-search indexing loop (idempotent).
    pub async fn content_search_start_historic_indexing(
        &self,
    ) -> MailContextResult<ContentSearchStartOutcome> {
        if let Some(service) = self.content_search_historic_indexing() {
            let ctx = self.as_arc();
            let driver = service.driver();
            driver.start(ctx).await
        } else {
            tracing::warn!("Content Search Service not available");
            Ok(ContentSearchStartOutcome::NoWork)
        }
    }

    /// Persist the content-search enable preference.
    pub async fn content_search_set_historic_indexing_enabled(
        &self,
        enabled: bool,
    ) -> MailContextResult<()> {
        if let Some(service) = self.content_search_historic_indexing() {
            let ctx = self.as_arc();
            let driver = service.driver();
            driver.set_enabled(ctx, enabled).await
        } else {
            tracing::warn!("Content Search Service not available");
            Ok(())
        }
    }

    /// Cancel historic indexing; optionally wipe local index data.
    pub async fn content_search_cancel_historic_indexing(
        &self,
        clear_data: bool,
    ) -> MailContextResult<()> {
        if let Some(service) = self.content_search_historic_indexing() {
            let ctx = self.as_arc();
            let driver = service.driver();
            driver.cancel_indexing(ctx, clear_data).await
        } else {
            tracing::warn!("Content Search Service not available");
            Ok(())
        }
    }

    /// Wipe locally-persisted content-search data; cancels an in-flight run first.
    pub async fn content_search_clear_historic_indexing_data(&self) -> MailContextResult<()> {
        if let Some(service) = self.content_search_historic_indexing() {
            let ctx = self.as_arc();
            let driver = service.driver();
            driver.clear_local_data(ctx).await
        } else {
            tracing::warn!("Content Search Service not available");
            Ok(())
        }
    }

    /// Get `MailUserContext` for each logged in account.
    ///
    pub async fn all_mail_user_ctxs(&self) -> MailContextResult<Vec<Arc<Self>>> {
        self.mail_context.get_all_logged_in_user_ctx().await
    }

    pub async fn other_mail_user_ctxs(&self) -> MailContextResult<Vec<Arc<Self>>> {
        self.mail_context
            .get_other_logged_in_user_ctx(self.session_id())
            .await
    }

    pub fn user_id(&self) -> &UserId {
        self.user_context.user_id()
    }

    pub fn session_id(&self) -> &SessionId {
        self.user_context.session_id()
    }

    pub(crate) fn rsvp_service(&self) -> &RsvpService {
        self.get_service::<RsvpService>()
    }

    pub fn attachment_cache_state(&self) -> &AttachmentCacheState {
        self.get_service::<AttachmentCacheState>()
    }

    pub async fn user(&self) -> MailContextResult<User> {
        let mail_stash = self.user_stash();
        let tether = mail_stash.connection();
        let user_id = self.user_id();
        let real_user = User::load(user_id.clone(), &tether)
            .await?
            .ok_or_else(|| MailContextError::Other(anyhow!("Missing User, this is a bug.")))?;

        Ok(real_user)
    }

    pub async fn account_details(&self) -> MailContextResult<AccountDetails> {
        let account_details = self.user_context.account_details().await?;
        Ok(account_details)
    }

    pub async fn user_settings(&self) -> MailContextResult<UserSettings> {
        Ok(self.user_context.user_settings().await?)
    }

    /// Loads the send preferences of the recipient with the given email address.
    ///
    /// [`SendPreferences`] contains the send preferences for sending an email to the given recipient
    /// including encryption/signing/formatting options and the encryption key.
    /// The send preferences are used to build the request for sending emails via Proton.
    /// [internal confluence docs](https://protonag.atlassian.net/wiki/spaces/MAILFE/pages/53117391/Send+preferences+for+outgoing+email)
    /// This information is collected from the keys returned by the API, contact vCard data,
    /// sender mail settings, and composer preferences.
    ///
    #[allow(clippy::too_many_arguments)]
    pub async fn recipient_send_preferences<P>(
        &self,
        pgp: &P,
        tether: &Tether,
        email: PrivateEmailRef<'_>,
        settings: CryptoMailSettings,
        composer_preference: ComposerPreference,
        fetch_policy: PublicAddressKeyApiFetchPolicy,
        contact_fetch_policy: PublicAddressKeyContactFetchPolicy,
    ) -> MailContextResult<SendPreferences<P::PublicKey>>
    where
        P: PGPProviderSync,
    {
        let encryption_time = crypto_clock::server_crypto_clock().unix_time();
        let address_key_selector = self
            .crypto_key_service()
            .load_with_tether(self.user_context(), tether)
            .address_keys_for_email(
                pgp,
                email.as_clear_text_str(),
                false,
                fetch_policy,
                contact_fetch_policy,
            )
            .await?;

        let encryption_preferences =
            address_key_selector.for_inbox_encryption(true, settings, encryption_time)?;

        let send_preferences =
            SendPreferences::from_preferences(encryption_preferences, composer_preference);

        Ok(send_preferences)
    }

    /// Loads the public keys required to verify a sender's cryptographic signature.
    ///
    /// Sender verification should be loaded when verifying signatures.
    /// This method gathers the sender's public keys from both the API and stored contacts.
    /// It then filters out any invalid keys according to Proton's key management policies,
    /// ensuring only valid keys are available for verification. Further, the result includes
    /// sender key information for potential UI indications.
    pub async fn sender_verification_preferences<P>(
        &self,
        pgp: &P,
        tether: &Tether,
        email: PrivateEmailRef<'_>,
        fetch_policy: PublicAddressKeyApiFetchPolicy,
        contact_fetch_policy: PublicAddressKeyContactFetchPolicy,
    ) -> MailContextResult<VerificationPreferences<P::PublicKey>>
    where
        P: PGPProviderSync,
    {
        let address_key_selector = self
            .crypto_key_service()
            .load_with_tether(self.user_context(), tether)
            .address_keys_for_email(
                pgp,
                email.as_clear_text_str(),
                true,
                fetch_policy,
                contact_fetch_policy,
            )
            .await?;

        let verification_preferences = address_key_selector.for_signature_verification();

        Ok(verification_preferences)
    }

    pub fn crypto_key_service(&self) -> &CryptoKeyService {
        self.user_context().crypto_key_service()
    }

    pub async fn new_password_change_flow(&self) -> MailContextResult<PasswordFlow> {
        let user = self.user().await?;
        let session = self.session().to_owned();
        let account = self.user_context.core_account().await?;
        let tfa_mode = account.second_factor_mode.unwrap_or_default();
        let mbp_mode = account.password_mode.unwrap_or_default();

        let key_secret = (session.expose_key_secret().await)
            .map(|s| s.expose_secret().to_owned())
            .unwrap(); // TODO: handle error

        Ok(PasswordFlow::new(
            session,
            user.email,
            user.keys.into(),
            key_secret,
            tfa_mode,
            mbp_mode,
        ))
    }

    pub async fn logout(&self) -> MailContextResult<()> {
        self.mail_context
            .logout_account(self.user_id().to_owned())
            .await?;

        Ok(())
    }

    /// Unlike [sign_out()] the user's metadata is preserved so it still shows up in the session
    /// picker.
    pub async fn logout_and_delete_data(&self) -> MailContextResult<()> {
        self.mail_context
            .logout_account_and_delete_user_data(self.user_id().clone())
            .await?;

        Ok(())
    }

    /// Sign this user out.
    ///
    /// Method will delete user's account and data from the device. The user will no longer
    /// be available in the session picker.
    ///
    pub async fn sign_out(&self) -> MailContextResult<()> {
        self.mail_context
            .delete_account(self.user_id().clone())
            .await?;

        Ok(())
    }

    /// Sign out from all accounts.
    ///
    /// This method will perform:
    /// * Each user logout
    /// * Clear all user data
    ///
    /// There are sevral layers to this function in which most of them
    /// are non failing and retrying in cases where we could fail.
    ///
    /// ### Notes
    ///
    /// There are no guarantees to clear everything especially on
    /// operating systems which locks files (looking at you Windows)
    /// though it will make best effort to get rid of any information
    /// app has stored over the course of its life.
    ///
    pub async fn sign_out_all(&self) -> MailContextResult<()> {
        let all_ctxs = self.all_mail_user_ctxs().await?;

        for ctx in all_ctxs {
            // If for any reason we fail to sign out account it will
            // be brought down anyway by tear_down in the next step
            // which also will get rid of key which is essential to
            // read data from API
            let _ = ctx
                .sign_out()
                .await
                .inspect_err(|e| tracing::error!("Could not remove account, `{e}`"));
        }

        Ok(())
    }

    pub async fn ping(&self) -> MailContextResult<()> {
        self.user_context
            .session()
            .get_tests_ping(None, None)
            .await?;
        Ok(())
    }

    pub fn connection_status(&self) -> ConnectionStatus {
        self.user_context.connection_status()
    }

    pub fn event_subscriber(
        &self,
    ) -> impl EventSubscriber<EventManagerContext, MailEventSourceV5> + 'static {
        MailEventV5Subscriber::new(Weak::clone(&self.this))
    }

    pub async fn queue_prefetch_jobs(
        self: &Arc<Self>,
        jobs: Vec<PrefetchJob>,
    ) -> MailContextResult<()> {
        if jobs.is_empty() {
            return Ok(());
        }

        let prefetch_service = self.get_service::<PrefetchService>();

        if let Some(sender) = prefetch_service.notify.get() {
            sender.send_async(jobs).await.map_err(|_| {
                MailContextError::Other(anyhow!("Failed to send prefetch signal to prefetcher"))
            })?;

            Ok(())
        } else {
            let (sender, receiver) = flume::bounded(DEFAULT_PREFETCH_BOUND);

            prefetch_service.notify.set(sender).map_err(|e| {
                MailContextError::Other(anyhow!("Failed to set prefetch sender: {e:?}"))
            })?;

            Prefetch::initialize(self.clone(), receiver).await;

            prefetch_service
                .notify
                .wait()
                .send_async(jobs)
                .await
                .map_err(|_| {
                    MailContextError::Other(anyhow!("Failed to send prefetch signal to prefetcher"))
                })?;

            Ok(())
        }
    }

    /// Get the path to the attachment staging folder.
    ///
    /// Attachment staging is used by mobile to place attachment files so they can
    /// be consumed later by the SDK. We can't directly use file system paths
    pub fn attachment_staging_path(&self) -> PathBuf {
        self.mail_context
            .mail_cache_path_for(self.user_id())
            .join("attachment-staging")
    }

    /// See [`UserContext::spawn()`].
    pub fn spawn<F>(&self, task: F) -> JoinHandle<F::Output>
    where
        F: Future<Output: Send> + Send + 'static,
    {
        self.user_context
            .spawn_cancellable(self.cancellation_token.clone(), task)
    }

    /// See [`Self::spawn()`].
    pub fn spawn_ex<Fn, Fut>(&self, task: Fn) -> JoinHandle<Fut::Output>
    where
        Fn: FnOnce(Arc<Self>) -> Fut,
        Fut: Future<Output: Send> + Send + 'static,
    {
        self.spawn(task(self.as_arc()))
    }

    pub async fn has_unsent_messages(&self) -> Result<bool, MailContextError> {
        Ok(self
            .action_queue()
            .typed_actions_count::<crate::actions::draft::Send>()
            .await?
            != 0)
    }

    pub async fn has_actions_in_queue(&self) -> Result<bool, MailContextError> {
        Ok(self.action_queue().queued_actions_count().await? != 0)
    }

    pub fn http_client(&self) -> &reqwest::Client {
        self.mail_context().http_client()
    }

    pub fn network_monitor_service(&self) -> &NetworkMonitorService {
        self.mail_context.network_monitor_service()
    }

    pub fn issue_reporter_service(&self) -> &UserIssueReporterService {
        self.user_context.issue_reporter_service()
    }

    pub fn image_loader(&self) -> &ImageLoader {
        self.get_service()
    }

    #[must_use]
    pub fn create_child_cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.child_token()
    }
}

impl Drop for MailUserContext {
    fn drop(&mut self) {
        self.cancellation_token.cancel(); //broadcast
        //Explicit termination of queues to ensure all tasks are cancelled
        if let Some(service) = self.get_service_opt::<DefaultQueueExecutor>() {
            service.terminate();
        }
        if let Some(service) = self.get_service_opt::<EventPollQueueExecutor>() {
            service.terminate();
        }
        if let Some(service) = self.get_service_opt::<SendQueueExecutorPool>() {
            service.terminate();
        }

        let user_id = self.user_id();
        let session_id = self.session_id();
        tracing::info!(?user_id, ?session_id, "Dropping MailUserContext");
        self.get_service::<EventSubscriberList>()
            .clear_subscribers(self);
    }
}

impl Spawner for MailUserContext {
    fn spawn_task<F>(&self, f: F) -> JoinHandle<F::Output>
    where
        F: Future<Output: Send> + Send + 'static,
    {
        self.spawn(f)
    }
}
