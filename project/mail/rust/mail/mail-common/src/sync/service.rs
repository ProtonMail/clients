use std::sync::{Arc, Weak};

use mail_core_common::services::UserEventService;
use mail_event_service::EventStream;
use mail_task_service::TaskService;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::sync::sync_context::SyncContext;
use crate::{BackwardSyncDriver, MailContextError, SyncBatch, SyncSettings};

#[derive(Debug, Clone)]
pub enum SyncEvent {
    Started,
    Stopped,
    Completed(SyncOutcome),
}

#[derive(Debug, Clone)]
pub enum SyncOutcome {
    Success,
    RetryableFailure(String),
    CriticalFailure(String),
}

pub struct SyncService {
    tx: flume::Sender<SyncServiceMessage>,
}

impl SyncService {
    pub fn new<C, D>(
        task_service: &TaskService,
        event_service: &UserEventService,
        driver: D,
        ctx: Weak<C>,
    ) -> Self
    where
        C: SyncContext,
        D: BackwardSyncDriver,
    {
        let (tx, rx) = flume::bounded(4);

        event_service.register_with_capacity::<SyncEvent>(4);
        let event_stream = event_service
            .subscribe::<SyncEvent>()
            .expect("Eevent was added above, this should exist");
        let mut worker = SyncServiceWorker {
            ctx,
            driver: Arc::new(driver),
            rx,
            back_sync_token: None,
            event_stream,
        };

        task_service.spawn(async move {
            worker.run().await;
        });

        Self { tx }
    }

    pub async fn start(&self) -> Result<(), MailContextError> {
        self.act(SyncServiceMessage::StartSync).await?
    }

    pub async fn stop(&self) -> Result<(), MailContextError> {
        self.act(SyncServiceMessage::StopSync).await?
    }

    async fn act<T>(
        &self,
        closure: impl FnOnce(oneshot::Sender<T>) -> SyncServiceMessage,
    ) -> Result<T, MailContextError> {
        let (rtx, rrx) = oneshot::channel();
        self.tx
            .send_async(closure(rtx))
            .await
            .map_err(|_| MailContextError::TaskCancelled)?;

        rrx.await.map_err(|_| MailContextError::TaskCancelled)
    }
}

enum SyncServiceMessage {
    StartSync(oneshot::Sender<Result<(), MailContextError>>),
    StopSync(oneshot::Sender<Result<(), MailContextError>>),
}

struct SyncServiceWorker<C: SyncContext, D: BackwardSyncDriver> {
    ctx: Weak<C>,
    driver: Arc<D>,
    rx: flume::Receiver<SyncServiceMessage>,
    back_sync_token: Option<CancellationToken>,
    event_stream: EventStream<SyncEvent>,
}

impl<C: SyncContext, D: BackwardSyncDriver> SyncServiceWorker<C, D> {
    async fn run(&mut self) {
        loop {
            tokio::select! {
                r = self.rx.recv_async() => {
                    let Ok(msg) = r else {
                        return;
                    };

                    let Some(ctx) = self.ctx.upgrade() else {
                        return;
                    };

                    self.on_message(&ctx, msg).await;
                }
                r = self.event_stream.next() => {
                    let Ok(event) = r else {
                        return;
                    };

                    let Some(ctx) = self.ctx.upgrade() else {
                        return;
                    };

                    self.on_event(&ctx, event).await;
                }
            }
        }
    }

    async fn on_message(&mut self, ctx: &Arc<C>, msg: SyncServiceMessage) {
        match msg {
            SyncServiceMessage::StartSync(sender) => {
                let result = self.start_sync(ctx).await;
                let _ = sender.send(result);
            }

            SyncServiceMessage::StopSync(sender) => {
                let result = self.stop_sync(ctx).await;
                let _ = sender.send(result);
            }
        }
    }

    async fn on_event(&mut self, ctx: &Arc<C>, event: SyncEvent) {
        match event {
            SyncEvent::Started | SyncEvent::Stopped => {
                // Do nothing
            }
            SyncEvent::Completed(SyncOutcome::Success) => {
                self.back_sync_token = None;
            }
            SyncEvent::Completed(
                SyncOutcome::RetryableFailure(error) | SyncOutcome::CriticalFailure(error),
            ) => {
                tracing::error!("Sync Failure: {error}, stopping sync");
                if let Err(e) = self.stop_sync(ctx).await {
                    tracing::error!("Failed to stop sync after failure: {e}")
                }
            }
        }
    }

    async fn start_sync(&mut self, ctx: &Arc<C>) -> Result<(), MailContextError> {
        if self.back_sync_token.is_some() {
            tracing::info!("Sync already running");
            return Ok(());
        }
        tracing::info!("Starting Sync");
        let mut tether = ctx.stash().connection();
        let settings = tether
            .write_tx(async |tx| SyncSettings::get_or_create(tx).await)
            .await?;

        if let Some(timestamp) = settings.backward_sync_complete {
            tracing::info!("Backward sync already completed on {}", timestamp);
            return Ok(());
        }

        ctx.user_event_service().publish(SyncEvent::Started);

        tracing::info!("Starting backward sync");
        // This is not really an error worth notifying to clients, it just updates some metadata
        if let Err(e) = tether
            .write_tx(async |tx| SyncSettings::mark_backward_sync_start(tx).await)
            .await
        {
            tracing::warn!("Failed to update sync start status: {e}");
        };

        let last_synced_batch = SyncBatch::find_oldest_batch(&tether).await?;

        let token = ctx.child_cancellation_token();
        let ctx_cloned = ctx.clone();
        let driver = self.driver.clone();
        ctx.task_service()
            .spawn_cancellable(token.clone(), async move {
                let api = ctx_cloned.api();
                let stash = ctx_cloned.stash();
                let queue = ctx_cloned.queue();
                let search_service = ctx_cloned.search_service();
                let network_observer = ctx_cloned.network_status_observer();

                match driver
                    .sync_loop(
                        last_synced_batch,
                        api,
                        stash,
                        queue,
                        search_service,
                        network_observer,
                    )
                    .await
                {
                    Ok(()) => {
                        let mut tether = stash.connection();
                        // A failed write here just means we'll redo backward sync on next
                        // start — not worth surfacing to clients. Either way the run is
                        // logically complete, so publish Complete unconditionally.
                        if let Err(e) = tether
                            .write_tx(async |tx| {
                                SyncSettings::mark_backward_sync_complete(tx).await
                            })
                            .await
                        {
                            tracing::warn!("Failed to update sync completed: {e}");
                        };
                        ctx_cloned
                            .user_event_service()
                            .publish(SyncEvent::Completed(SyncOutcome::Success));
                    }
                    Err(MailContextError::Api(e))
                        if e.is_network_failure()
                            || e.is_server_failure()
                            || e.is_auth_failure() =>
                    {
                        tracing::error!("Backard sync failed with retryable error: {e}");
                        ctx_cloned
                            .user_event_service()
                            .publish(SyncEvent::Completed(SyncOutcome::RetryableFailure(
                                e.to_string(),
                            )));
                    }
                    Err(e) => {
                        tracing::error!("Backard sync error: {e}");
                        ctx_cloned
                            .user_event_service()
                            .publish(SyncEvent::Completed(SyncOutcome::CriticalFailure(
                                e.to_string(),
                            )));
                    }
                }
            });
        self.back_sync_token = Some(token);

        Ok(())
    }

    async fn stop_sync(&mut self, ctx: &Arc<C>) -> Result<(), MailContextError> {
        if let Some(token) = self.back_sync_token.take() {
            tracing::info!("Stopping Sync");
            token.cancel();
            ctx.user_event_service().publish(SyncEvent::Stopped);
        } else {
            tracing::info!("No sync currently running");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::dependencies::DependencyApi;
    use crate::sync::driver::{BackwardSync, BackwardSyncApi, BackwardSyncParams};
    use mail_account_api::protocol::proton::GetAddressResponse;
    use mail_action_queue::queue::TokioTaskSpawner;
    use mail_api::services::proton::prelude::{GetMessagesOptions, GetMessagesResponse};
    use mail_api::services::proton::responses::RunningTasks;
    use mail_api_labels::LabelId;
    use mail_common::test_utils::db::new_test_connection_file;
    use mail_core_api::service::{ApiServiceError, ApiServiceResult};
    use mail_core_api::services::proton::AddressId;
    use mail_core_common::datatypes::LabelType;
    use mail_core_common::models::{Label, LabelError};
    use mail_core_common::test_utils::test_context::test_network_monitor_service_config;
    use mail_event_service::EventStream;
    use mail_network_monitor_service::{NetworkMonitorService, NetworkStatusObserver};
    use mail_search::MailSearchService;
    use mail_stash::UserDb;
    use mail_stash::stash::Stash;
    use mail_task_service::{BackgroundAwareTaskService, TaskService};
    use std::time::Duration;
    use tempfile::TempDir;

    /// Driver whose `sync_loop` never resolves until the spawned task is
    /// aborted via the worker's cancellation token.
    struct HangingDriver;

    impl BackwardSyncDriver for HangingDriver {
        fn sync_loop<API: BackwardSyncApi>(
            &self,
            _previous_batch: Option<SyncBatch>,
            _api: &API,
            _stash: &Stash<UserDb>,
            _queue: &mail_action_queue::queue::Queue<UserDb>,
            _search_service: Option<&MailSearchService>,
            _network_status_observer: NetworkStatusObserver,
        ) -> impl std::future::Future<Output = Result<(), MailContextError>> + Send {
            std::future::pending::<Result<(), MailContextError>>()
        }
    }

    mockall::mock! {
        pub Api {}
        impl DependencyApi for Api {
            async fn get_address_by_id(&self, id: AddressId)
                -> ApiServiceResult<GetAddressResponse>;
            async fn fetch_labels_by_ids(&self, ids: Vec<LabelId>)
                -> Result<Vec<Label>, LabelError>;
            async fn fetch_labels(&self, label_types: Vec<LabelType>)
                -> Result<Vec<Label>, LabelError>;
        }
        impl BackwardSyncApi for Api {
            async fn get_messages(&self, options: GetMessagesOptions)
                -> ApiServiceResult<GetMessagesResponse>;
        }
    }

    fn empty_messages_response() -> GetMessagesResponse {
        GetMessagesResponse {
            messages: vec![],
            tasks_running: RunningTasks::NotKnown,
            stale: false,
            total: 0,
        }
    }

    struct TestSyncContext {
        stash: Stash<UserDb>,
        api: MockApi,
        queue: mail_action_queue::queue::Queue<UserDb>,
        task_service: BackgroundAwareTaskService,
        event_service: UserEventService,
        network_service: NetworkMonitorService,
        token: CancellationToken,
    }

    impl SyncContext for TestSyncContext {
        type Api = MockApi;
        fn stash(&self) -> &Stash<UserDb> {
            &self.stash
        }
        fn api(&self) -> &Self::Api {
            &self.api
        }
        fn queue(&self) -> &mail_action_queue::queue::Queue<UserDb> {
            &self.queue
        }
        fn search_service(&self) -> Option<&MailSearchService> {
            None
        }
        fn task_service(&self) -> &BackgroundAwareTaskService {
            &self.task_service
        }
        fn user_event_service(&self) -> &UserEventService {
            &self.event_service
        }
        fn network_status_observer(&self) -> NetworkStatusObserver {
            self.network_service.network_status_observer()
        }
        fn child_cancellation_token(&self) -> CancellationToken {
            self.token.child_token()
        }
    }

    struct TestContext {
        ctx: Arc<TestSyncContext>,
        _db_dir: TempDir,
    }

    impl TestContext {
        async fn new(mk_mocks: impl FnOnce(&mut MockApi)) -> TestContext {
            let (stash, db_dir) = new_test_connection_file().await;
            let queue = mail_action_queue::queue::Queue::new(stash.clone(), TokioTaskSpawner)
                .await
                .unwrap();
            let task_service = BackgroundAwareTaskService::new(
                TaskService::new(tokio::runtime::Handle::current()).unwrap(),
            );
            let event_service = UserEventService::new();
            event_service.register_with_capacity::<SyncEvent>(4);
            let network_service = NetworkMonitorService::new(test_network_monitor_service_config());

            let mut api = MockApi::new();
            mk_mocks(&mut api);

            TestContext {
                ctx: Arc::new(TestSyncContext {
                    stash,
                    api,
                    queue,
                    task_service,
                    event_service,
                    network_service,
                    token: CancellationToken::new(),
                }),
                _db_dir: db_dir,
            }
        }

        fn subscribe(&self) -> EventStream<SyncEvent> {
            self.ctx
                .user_event_service()
                .subscribe::<SyncEvent>()
                .unwrap()
        }

        fn make_service<D: BackwardSyncDriver>(&self, driver: D) -> (SyncService, TaskService) {
            let task_service = TaskService::new(tokio::runtime::Handle::current()).unwrap();
            let service = SyncService::new(
                &task_service,
                self.ctx.user_event_service(),
                driver,
                Arc::downgrade(&self.ctx),
            );
            (service, task_service)
        }

        fn make_default_service(&self) -> (SyncService, TaskService) {
            self.make_service(BackwardSync::new(BackwardSyncParams::default()))
        }
    }

    /// Returns the next SyncEvent or panics on timeout / closed stream.
    async fn next_event(stream: &mut EventStream<SyncEvent>) -> SyncEvent {
        tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("timeout waiting for event")
            .expect("event stream closed")
    }

    /// Drain events until one matching `pred` arrives. Panics on timeout.
    async fn wait_for<F: Fn(&SyncEvent) -> bool>(
        stream: &mut EventStream<SyncEvent>,
        pred: F,
    ) -> SyncEvent {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let evt = next_event(stream).await;
                if pred(&evt) {
                    return evt;
                }
            }
        })
        .await
        .unwrap()
    }

    async fn assert_no_event(stream: &mut EventStream<SyncEvent>, dur: Duration) {
        let evt = tokio::time::timeout(dur, stream.next()).await;
        assert!(evt.is_err(), "expected no event, got {evt:?}");
    }

    #[tokio::test]
    async fn start_sync_is_noop_when_backward_sync_already_complete() {
        let ctx = TestContext::new(|_| {}).await;

        let mut tether = ctx.ctx.stash().connection();
        tether
            .write_tx(async |tx| {
                SyncSettings::get_or_create(tx).await?;
                SyncSettings::mark_backward_sync_complete(tx).await
            })
            .await
            .unwrap();

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_default_service();
        service.start().await.unwrap();

        assert_no_event(&mut stream, Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn start_sync_is_noop_when_sync_already_running() {
        // HangingDriver keeps sync_loop unresolved; the worker's
        // `back_sync_token` therefore stays `Some` across the second
        // `service.start()`, exercising the short-circuit branch.
        let ctx = TestContext::new(|_| {}).await;

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_service(HangingDriver);
        service.start().await.unwrap();
        wait_for(&mut stream, |e| matches!(e, SyncEvent::Started)).await;

        // Second start: short-circuit → no second Started event.
        service.start().await.unwrap();
        assert_no_event(&mut stream, Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn stop_sync_is_noop_when_no_sync_running() {
        let ctx = TestContext::new(|_| {}).await;

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_default_service();
        service.stop().await.unwrap();

        assert_no_event(&mut stream, Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn start_sync_publishes_started_event() {
        let ctx = TestContext::new(|api| {
            api.expect_get_messages()
                .returning(|_| Ok(empty_messages_response()));
        })
        .await;

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_default_service();
        service.start().await.unwrap();

        let evt = next_event(&mut stream).await;
        assert!(
            matches!(evt, SyncEvent::Started),
            "expected Started, got {evt:?}"
        );
    }

    #[tokio::test]
    async fn stop_sync_publishes_stopped_event() {
        // HangingDriver keeps the spawned sync alive until `service.stop()`
        // cancels the back_sync_token, exercising the Stopped publish branch.
        let ctx = TestContext::new(|_| {}).await;

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_service(HangingDriver);
        service.start().await.unwrap();
        wait_for(&mut stream, |e| matches!(e, SyncEvent::Started)).await;

        service.stop().await.unwrap();
        wait_for(&mut stream, |e| matches!(e, SyncEvent::Stopped)).await;
    }

    #[tokio::test]
    async fn start_sync_marks_backward_sync_start_in_settings() {
        let ctx = TestContext::new(|api| {
            api.expect_get_messages()
                .returning(|_| Ok(empty_messages_response()));
        })
        .await;

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_default_service();
        service.start().await.unwrap();
        // Wait until Started fires so start_sync has run past the settings write.
        wait_for(&mut stream, |e| matches!(e, SyncEvent::Started)).await;

        let tether = ctx.ctx.stash().connection();
        let settings = SyncSettings::get(&tether)
            .await
            .unwrap()
            .expect("settings row should exist after start_sync");
        assert!(
            settings.backward_sync_start.is_some(),
            "backward_sync_start should be set after start_sync"
        );
    }

    #[tokio::test]
    async fn start_sync_passes_find_oldest_batch_as_previous_batch() {
        let (called_tx, called_rx) = tokio::sync::oneshot::channel();
        let called_tx = std::sync::Mutex::new(Some(called_tx));
        let ctx = TestContext::new(|api| {
            api.expect_get_messages()
                .withf(|options| {
                    options.end_id
                        == Some(mail_api::services::proton::prelude::MessageId::from(
                            "seed-end",
                        ))
                        && options.end == Some(1_500)
                })
                .returning(move |_| {
                    if let Some(tx) = called_tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                    Ok(empty_messages_response())
                });
        })
        .await;

        // Seed a batch in the DB so find_oldest_batch returns Some.
        let mut tether = ctx.ctx.stash().connection();
        tether
            .write_tx::<_, _, mail_stash::stash::StashError>(async |tx| {
                SyncBatch::create(
                    mail_api::services::proton::prelude::MessageId::from("seed-begin"),
                    mail_core_common::datatypes::UnixTimestamp::new(2_000),
                    mail_api::services::proton::prelude::MessageId::from("seed-end"),
                    mail_core_common::datatypes::UnixTimestamp::new(1_500),
                    tx,
                )
                .await
            })
            .await
            .unwrap();
        drop(tether);

        let (service, _task) = ctx.make_default_service();
        service.start().await.unwrap();

        tokio::time::timeout(Duration::from_secs(2), called_rx)
            .await
            .expect("spawned task never called get_messages with the seeded anchor")
            .unwrap();
    }

    #[tokio::test]
    async fn spawned_sync_marks_backward_sync_complete_on_success() {
        let ctx = TestContext::new(|api| {
            api.expect_get_messages()
                .returning(|_| Ok(empty_messages_response()));
        })
        .await;

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_default_service();
        service.start().await.unwrap();

        // The spawned task publishes Complete after marking the settings row.
        wait_for(&mut stream, |e| {
            matches!(e, SyncEvent::Completed(SyncOutcome::Success))
        })
        .await;

        let tether = ctx.ctx.stash().connection();
        let settings = SyncSettings::get(&tether)
            .await
            .unwrap()
            .expect("settings row");
        assert!(
            settings.backward_sync_complete.is_some(),
            "backward_sync_complete should be set"
        );
    }

    #[tokio::test]
    async fn spawned_sync_publishes_retryable_failure_on_server_error() {
        let ctx = TestContext::new(|api| {
            api.expect_get_messages().returning(|_| {
                Err(ApiServiceError::InternalServerError(
                    "boom".to_owned(),
                    None,
                ))
            });
        })
        .await;

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_default_service();
        service.start().await.unwrap();

        wait_for(&mut stream, |e| {
            matches!(e, SyncEvent::Completed(SyncOutcome::RetryableFailure(_)))
        })
        .await;
    }

    #[tokio::test]
    async fn spawned_sync_publishes_critical_failure_on_other_error() {
        let ctx = TestContext::new(|api| {
            api.expect_get_messages()
                .returning(|_| Err(ApiServiceError::ResponseError("malformed json".to_owned())));
        })
        .await;

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_default_service();
        service.start().await.unwrap();

        wait_for(&mut stream, |e| {
            matches!(e, SyncEvent::Completed(SyncOutcome::CriticalFailure(_)))
        })
        .await;
    }

    #[tokio::test]
    async fn complete_event_clears_back_sync_token() {
        let ctx = TestContext::new(|api| {
            api.expect_get_messages()
                .returning(|_| Ok(empty_messages_response()));
        })
        .await;

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_default_service();
        service.start().await.unwrap();

        wait_for(&mut stream, |e| {
            matches!(e, SyncEvent::Completed(SyncOutcome::Success))
        })
        .await;

        // No event should be produced since a complete sync implies stopped.
        service.stop().await.unwrap();
        assert_no_event(&mut stream, Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn retryable_failure_event_triggers_stop() {
        let ctx = TestContext::new(|api| {
            api.expect_get_messages().returning(|_| {
                Err(ApiServiceError::InternalServerError(
                    "boom".to_owned(),
                    None,
                ))
            });
        })
        .await;

        let mut stream = ctx.subscribe();
        let (service, _task) = ctx.make_default_service();
        service.start().await.unwrap();

        wait_for(&mut stream, |e| {
            matches!(e, SyncEvent::Completed(SyncOutcome::RetryableFailure(_)))
        })
        .await;
        wait_for(&mut stream, |e| matches!(e, SyncEvent::Stopped)).await;
    }
}
