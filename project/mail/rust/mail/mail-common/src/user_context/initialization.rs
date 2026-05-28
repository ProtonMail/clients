use crate::models::{
    CustomSettings, IncomingDefault, LabelWithCounters, MailSettings, StoreLabelCounters,
};
use crate::{MailContextError, MailContextResult, MailUserContext, NewMailUserContextOptions};
use futures::try_join;
use mail_core_common::datatypes::{InitializationKey, InitializedComponentState};
use mail_core_common::models::{
    Address, DependencyInitializationError, InitializationError, InitializationWatcher,
    InitializedComponent, User, initialize_contacts,
};

use core_event_loop::EventLoopError;
use mail_core_common::services::{EventLoopService, InitializationService};
use mail_issue_reporter_service::{IssueLevel, IssueReportKeys, issue_report_keys_from_error};
use mail_stash::params;
use mail_task_service::TaskService;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum MailUserContextLoadingStage {
    UserSettings,
    MailSettings,
    CustomSettings,
    Addresses,
    Events,
    Labels,
    Counters,
    Contacts,
}

impl MailUserContext {
    pub const CONTEXT_INIT_KEY: InitializationKey = InitializationKey::new("mail_user_context");

    /// Initialize the mail user context, running all the necessary syncs to ensure the context is ready to be used.
    /// Syncs are mostly run in the parallel, but updating message & conversation count are dependent on labels, so it is run in sequence.
    ///
    /// # Warning
    ///
    /// This function probably should not be called explicitly.
    /// It is called automatically during user context session creation
    pub async fn initialize_async(
        ctx: Arc<Self>,
        options: NewMailUserContextOptions,
    ) -> Result<(), MailContextError> {
        let ctx_cloned = Arc::clone(&ctx);

        let result = ctx
            .get_service::<InitializationMediator>()
            .initialize(ctx_cloned, options)
            .await;

        match &result {
            Ok(()) => ctx.post_initialize(),
            Err(err) => {
                if !err.is_network_failure() {
                    ctx.issue_reporter_service().report(
                        IssueLevel::Error,
                        "Failed to initialize mail user context".into(),
                        issue_report_keys_from_error(err),
                    );
                }
            }
        }

        result
    }

    /// Checks whether initialization process finished suscesfully.
    ///
    pub async fn is_initialized(&self) -> Result<bool, MailContextError> {
        let tether = self.user_stash().connection();
        let state = InitializedComponent::state(Self::CONTEXT_INIT_KEY, &tether).await?;
        Ok(matches!(state, InitializedComponentState::Succeeded))
    }

    /// Wait for the `MailUserContext` to be initialized.
    pub(crate) async fn wait_on_initialized(
        &self,
        watcher: &InitializationWatcher,
    ) -> Result<(), DependencyInitializationError> {
        let tether = self.user_stash().connection();
        InitializedComponent::wait_for_dependencies(&[Self::CONTEXT_INIT_KEY], watcher, &tether)
            .await
    }

    /// Initialize a component.
    #[tracing::instrument(skip(handle))]
    async fn initial_sync_for<E>(
        stage: MailUserContextLoadingStage,
        handle: JoinHandle<Result<(), InitializationError<E>>>,
    ) -> Result<(), MailContextError>
    where
        E: std::fmt::Debug + Send + Sync + 'static,
        MailContextError: From<E>,
    {
        let t = Instant::now();
        debug!("Begin syncing for {stage:?}");

        let result = handle.await;
        let elapsed = t.elapsed();
        if elapsed > Duration::from_secs(1) {
            warn!("Slow sync for {stage:?}: {elapsed:?}");
        } else {
            debug!("Syncing {stage:?} took {elapsed:?}");
        }

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => match e {
                InitializationError::InitializationFailed(e) => {
                    let e = e.into();
                    error!("Failed to sync {e:?}");
                    Err(e)
                }
                InitializationError::Stash(e) => {
                    let e = e.into();
                    error!("Failed to sync {e:?}");
                    Err(e)
                }
            },
            Err(e) => {
                if e.is_cancelled() {
                    error!("Called while syncing {stage:?}");
                    Err(MailContextError::TaskCancelled)
                } else {
                    let e = e.into();
                    error!("Panicked while syncing {stage:?}: {e:?}");
                    Err(e)
                }
            }
        }
    }

    fn spawn_init<'a, T, F, Fut>(
        self: &'a Arc<Self>,
        watcher: &'a Arc<InitializationWatcher>,
        f: F,
    ) -> JoinHandle<T>
    where
        T: Send + 'static,
        F: FnOnce(Arc<Self>, Arc<InitializationWatcher>) -> Fut,
        Fut: Future<Output = T> + Send + 'static,
    {
        let ctx_clone = self.clone();
        let watcher_clone = watcher.clone();
        self.spawn(f(ctx_clone, watcher_clone))
    }

    /// Bootstraps that are too slow for the critical path and whose absence
    /// degrades UX gracefully (e.g. banner not shown, default encoding used).
    ///
    /// Triggered from [`Self::initialize_async`] so it fires on both first launch
    /// AND relaunch — the mediator early-returns when `CONTEXT_INIT_KEY` is
    /// already `Succeeded`, but a prior session may have been killed before its
    /// own post-init finished. Each helper is idempotent via its `INIT_KEY` state.
    pub(crate) fn post_initialize(self: &Arc<Self>) {
        self.start_incoming_default_background_load();
    }

    /// Some accounts have 70k+ rows; synchronous load blocks login for minutes.
    /// Read-side consumers degrade gracefully when the table is empty.
    fn start_incoming_default_background_load(self: &Arc<Self>) {
        let ctx = Arc::clone(self);
        self.spawn(async move {
            if let Err(e) = try_load_incoming_defaults(&ctx).await {
                error!("post_init: IncomingDefault load failed: {e:?}");
                if !e.is_network_failure() {
                    ctx.issue_reporter_service().report(
                        IssueLevel::Error,
                        "post_init: IncomingDefault load failed".into(),
                        issue_report_keys_from_error(&e),
                    );
                }
            }
        });
    }
}

async fn try_load_incoming_defaults(ctx: &Arc<MailUserContext>) -> Result<(), MailContextError> {
    let tether = ctx.user_stash().connection();
    if matches!(
        InitializedComponent::state(IncomingDefault::INIT_KEY, &tether).await?,
        InitializedComponentState::Succeeded
    ) {
        debug!("post_init: IncomingDefault already initialized, skipping");
        return Ok(());
    }

    let watcher = ctx
        .user_context
        .get_service::<InitializationService>()
        .initialization_watcher()
        .clone();
    let watcher_clone = watcher.clone();
    let watcher_task = ctx.spawn(async move { watcher_clone.task().await });

    let t0 = Instant::now();
    info!("post_init: IncomingDefault background load starting");
    let result = IncomingDefault::initialize(watcher, ctx.session(), ctx.user_stash()).await;
    watcher_task.abort();

    let elapsed = t0.elapsed();
    match result {
        Ok(()) => {
            if elapsed > Duration::from_secs(60) {
                warn!("post_init: slow IncomingDefault load: {elapsed:?}");
                ctx.issue_reporter_service().report(
                    IssueLevel::Warning,
                    "post_init: slow IncomingDefault load".into(),
                    IssueReportKeys::from([("elapsed".into(), format!("{elapsed:?}"))]),
                );
            } else {
                info!("post_init: IncomingDefault load done in {elapsed:?}");
            }
            Ok(())
        }
        Err(InitializationError::InitializationFailed(e)) => Err(e),
        Err(InitializationError::Stash(e)) => Err(e.into()),
    }
}

const EVENT_INIT_KEY: InitializationKey = InitializationKey::new("events");

async fn initialize_event_loop(
    watcher: Arc<InitializationWatcher>,
    ctx: &MailUserContext,
) -> Result<(), InitializationError<EventLoopError>> {
    let mail_stash = ctx.user_stash();

    InitializedComponent::initialize(
        watcher,
        EVENT_INIT_KEY,
        &[],
        mail_stash.connection(),
        async || {
            // This is a little bit of a hack. The way of how this
            // event loop initialization is currently written,
            // there is no way of initializing it with already having transaction.
            // We want to avoid the deadlock, and we do not depend on any dependencies.
            // So initializing it here is not really harmful, just weird.
            ctx.user_context()
                .get_service::<EventLoopService>()
                .event_poll()
                .initialize()
                .await?;

            Ok(())
        },
        |_tx, ()| Ok(()),
    )
    .await
}

type InitializerMessage = (
    Arc<MailUserContext>,
    NewMailUserContextOptions,
    tokio::sync::oneshot::Sender<MailContextResult<()>>,
);
/// This mediator makes sure that we only ever initialize the context in a serial fashion.
///
/// It is possible on mobile to trigger multiple context inits at the same time. Initialization
/// is funneled through here to make sure it's not being done concurrently.
pub(crate) struct InitializationMediator {
    sender: flume::Sender<InitializerMessage>,
}
impl InitializationMediator {
    const CHANNEL_CAPACITY: usize = 1;
    pub(crate) fn new(task_service: &TaskService) -> Self {
        let (sender, receiver) = flume::bounded::<InitializerMessage>(Self::CHANNEL_CAPACITY);
        task_service.spawn(async move { Self::background_loop(receiver).await });

        Self { sender }
    }

    async fn background_loop(receiver: flume::Receiver<InitializerMessage>) {
        while let Ok((ctx, options, sender)) = receiver.recv_async().await {
            let r = Self::initialize_context(ctx, options).await;
            _ = sender.send(r);
        }
    }

    /// Send an initialization request and wait for the result.
    pub(crate) async fn initialize(
        &self,
        ctx: Arc<MailUserContext>,
        options: NewMailUserContextOptions,
    ) -> MailContextResult<()> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        if self
            .sender
            .send_async((ctx, options, sender))
            .await
            .is_err()
        {
            error!("Failed to communicate with initializer mediator");
            return Err(MailContextError::InitMediatorError);
        };

        receiver.await.unwrap_or_else(|_| {
            error!("Failed to communicate with initializer mediator");
            Err(MailContextError::InitMediatorError)
        })
    }

    #[tracing::instrument(skip_all, fields(user_id=%ctx.user_id()))]
    async fn initialize_context(
        ctx: Arc<MailUserContext>,
        options: NewMailUserContextOptions,
    ) -> Result<(), MailContextError> {
        tracing::info!(
            "Initializing mail user context (force resync user={})",
            options.resync_user
        );

        if options.resync_user {
            let mut tether = ctx.user_stash().connection();

            tether
                .write_tx(async |tx| {
                    tx.execute(
                        "DELETE FROM initialized_components WHERE key = ? OR key = ? OR key = ?",
                        params![
                            User::INIT_KEY.0,
                            MailUserContext::CONTEXT_INIT_KEY.0,
                            IncomingDefault::INIT_KEY.0
                        ],
                    )
                    .await
                })
                .await?;
        }

        if ctx.is_initialized().await? {
            warn!("Context already initialized");
            return Ok(());
        }
        let watcher = ctx
            .user_context
            .get_service::<InitializationService>()
            .initialization_watcher()
            .clone();
        let watcher_clone = watcher.clone();
        let watcher_task_handle = ctx.spawn(async move { watcher_clone.task().await });

        let t0 = Instant::now();

        let labels = ctx.spawn_init(&watcher, |ctx, watcher| async move {
            LabelWithCounters::initialize(watcher, ctx.session(), ctx.user_stash()).await
        });
        let counters = ctx.spawn_init(&watcher, |ctx, watcher| async move {
            StoreLabelCounters::initialize(watcher, ctx.session(), ctx.user_stash()).await
        });
        let contacts = ctx.spawn_init(&watcher, |ctx, watcher| async move {
            initialize_contacts(watcher, ctx.session(), ctx.user_stash()).await
        });
        let event_loop = ctx.spawn_init(&watcher, |ctx, watcher| async move {
            initialize_event_loop(watcher, ctx.as_ref()).await
        });
        let user_settings = ctx.spawn_init(&watcher, |ctx, watcher| async move {
            User::initialize_with_settings(watcher, ctx.session(), ctx.user_stash()).await
        });
        let mail_settings = ctx.spawn_init(&watcher, |ctx, watcher| async move {
            MailSettings::initialize(watcher, ctx.session(), ctx.user_stash()).await
        });
        let custom_settings = ctx.spawn_init(&watcher, |ctx, watcher| async move {
            CustomSettings::initialize(
                watcher,
                ctx.user_id(),
                ctx.user_stash(),
                ctx.core_context().account_stash(),
            )
            .await
        });
        let addresses = ctx.spawn_init(&watcher, |ctx, watcher| async move {
            Address::initialize(watcher, ctx.session(), ctx.user_stash()).await
        });

        let abort_handles = vec![
            watcher_task_handle.abort_handle(),
            labels.abort_handle(),
            contacts.abort_handle(),
            counters.abort_handle(),
            event_loop.abort_handle(),
            user_settings.abort_handle(),
            mail_settings.abort_handle(),
            custom_settings.abort_handle(),
            addresses.abort_handle(),
        ];

        let res = try_join!(
            MailUserContext::initial_sync_for(MailUserContextLoadingStage::Labels, labels),
            MailUserContext::initial_sync_for(MailUserContextLoadingStage::Contacts, contacts),
            MailUserContext::initial_sync_for(MailUserContextLoadingStage::Counters, counters),
            MailUserContext::initial_sync_for(MailUserContextLoadingStage::Events, event_loop),
            MailUserContext::initial_sync_for(
                MailUserContextLoadingStage::UserSettings,
                user_settings
            ),
            MailUserContext::initial_sync_for(
                MailUserContextLoadingStage::MailSettings,
                mail_settings
            ),
            MailUserContext::initial_sync_for(
                MailUserContextLoadingStage::CustomSettings,
                custom_settings
            ),
            MailUserContext::initial_sync_for(MailUserContextLoadingStage::Addresses, addresses),
        );

        abort_handles.into_iter().for_each(|a| a.abort());

        match res {
            Ok(_) => {
                InitializedComponent::set_state(
                    MailUserContext::CONTEXT_INIT_KEY,
                    InitializedComponentState::Succeeded,
                    &mut ctx.user_stash().connection(),
                )
                .await?;

                debug!("Syncing Complete in {:?}", t0.elapsed());
                Ok(())
            }
            Err(e) => {
                InitializedComponent::set_state(
                    MailUserContext::CONTEXT_INIT_KEY,
                    InitializedComponentState::Failed,
                    &mut ctx.user_stash().connection(),
                )
                .await?;

                error!("Syncing Failed in {:?}", t0.elapsed());
                Err(e)
            }
        }
    }
}
