use std::num::NonZeroUsize;

use itertools::Itertools;
use mail_action_queue::action::ActionGroup;
use mail_action_queue::queue::Queue;
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api::services::proton::ProtonMail;
use mail_api::services::proton::prelude::{GetMessagesOptions, GetMessagesResponse};
use mail_api_labels::LabelId;
use mail_core_api::service::ApiServiceResult;
use mail_network_monitor_service::NetworkStatusObserver;
use mail_search::MailSearchService;
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::Stash;

use crate::datatypes::SystemLabelId;
use crate::datatypes::dependencies::{DependencyApi, DependencyFetcher};
use crate::models::{Message, MessageSyncDecision};
use crate::{MailContextError, SyncBatch};

pub trait BackwardSyncApi: DependencyApi {
    fn get_messages(
        &self,
        options: GetMessagesOptions,
    ) -> impl std::future::Future<Output = ApiServiceResult<GetMessagesResponse>> + Send;
}

impl BackwardSyncApi for mail_core_api::session::Session {
    async fn get_messages(
        &self,
        options: GetMessagesOptions,
    ) -> ApiServiceResult<GetMessagesResponse> {
        ProtonMail::get_messages(self, options).await
    }
}

// Abstraction so it is easier to test the background sync worker task.
pub trait BackwardSyncDriver: Send + Sync + 'static {
    fn sync_loop<API: BackwardSyncApi>(
        &self,
        previous_batch: Option<SyncBatch>,
        api: &API,
        stash: &Stash<UserDb>,
        queue: &Queue<UserDb>,
        search_service: Option<&MailSearchService>,
        network_status_observer: NetworkStatusObserver,
    ) -> impl std::future::Future<Output = Result<(), MailContextError>> + Send;
}

#[derive(Debug)]
pub struct BackwardSyncParams {
    pub page_size: NonZeroUsize,
    /// Metadata is fetched in `page_size` chunks, but it maybe
    /// better to store each sync chunk in smaller chuncks
    /// dependening on platform constraints. If not
    /// the `page_size` is used instead.
    pub chunk_split: Option<NonZeroUsize>,
}

const DEFAULT_PAGE_SIZE: usize = 200;

impl Default for BackwardSyncParams {
    fn default() -> Self {
        Self {
            page_size: NonZeroUsize::new(DEFAULT_PAGE_SIZE).expect("Should always work"),
            chunk_split: None,
        }
    }
}

pub struct BackwardSync {
    params: BackwardSyncParams,
}

impl BackwardSync {
    pub fn new(params: BackwardSyncParams) -> Self {
        Self { params }
    }

    #[tracing::instrument(skip_all)]
    pub async fn sync_next<API>(
        &self,
        previous_batch: Option<&SyncBatch>,
        api: &API,
        stash: &Stash<UserDb>,
        queue: &Queue<UserDb>,
        search_service: Option<&MailSearchService>,
    ) -> Result<Option<Vec<SyncBatch>>, MailContextError>
    where
        API: BackwardSyncApi,
    {
        let options = if let Some(batch) = previous_batch {
            tracing::info!(
                "Resuming {:?} from {:?}:{}",
                batch.id,
                batch.end_id,
                batch.end_time
            );

            GetMessagesOptions {
                label_id: Some(vec![LabelId::all_mail()]),
                desc: Some(true),
                end_id: Some(batch.end_id.clone()),
                end: Some(batch.end_time.as_u64()),
                page_size: self.params.page_size.get() as u64,
                ..Default::default()
            }
        } else {
            tracing::info!("No existing, batch starting from scratch");
            GetMessagesOptions {
                label_id: Some(vec![LabelId::all_mail()]),
                desc: Some(true),
                page_size: self.params.page_size.get() as u64,
                ..Default::default()
            }
        };

        let response = api.get_messages(options.clone()).await?;
        let mut messages = response.messages;
        if !messages.is_empty()
            && let Some(batch) = previous_batch
            && messages[0].id == batch.end_id
        {
            messages.remove(0);
        }

        tracing::debug!("Fetched {} message metadata", messages.len());

        if messages.is_empty() {
            return Ok(None);
        }

        let message_count = messages.len();

        let mut tether = stash.connection();

        let mut dependency_fetcher = DependencyFetcher::default();

        for metadata in &messages {
            dependency_fetcher
                .check_api_message_metadata(metadata, &tether)
                .await?;
        }

        dependency_fetcher
            .fetch_and_store(api, &mut tether, queue)
            .await
            .inspect_err(|e| tracing::error!("Failed to sync dependencies: {e}"))?;

        Ok(tether
            .write_tx(async |tx| {
                let mut batches = Vec::new();
                let chunk_size = self
                    .params
                    .chunk_split
                    .map(|v| v.get())
                    .unwrap_or(self.params.page_size.get());

                // `IntoChunks` is !Send, so collect eagerly before any `.await` —
                // otherwise sync_next's future cannot be spawned by SyncServiceWorker.
                let chunks: Vec<Vec<_>> = messages
                    .into_iter()
                    .chunks(chunk_size)
                    .into_iter()
                    .map(|c| c.collect())
                    .collect();
                for messages in chunks {
                    let (begin_id, begin_time, end_id, end_time) = {
                        let first = &messages[0];
                        let last = messages.last().expect("Should be set");
                        (first.id.clone(), first.time, last.id.clone(), last.time)
                    };
                    let mut rebase_changeset = RebaseChangeSet::default();
                    for metadata in messages {
                        if Message::sync_decision(&metadata, None, search_service, tx).await?
                            == MessageSyncDecision::Apply
                        {
                            let mut message = Message::from_api_metadata(metadata, tx).await?;
                            message.save(tx).await?;
                            rebase_changeset.add(message.id());
                        }
                    }

                    if let Err(e) = queue
                        .rebase_in(ActionGroup::default(), &rebase_changeset, tx)
                        .await
                    {
                        tracing::error!("Failed to rebase: {e}");
                    }

                    let new_batch =
                        SyncBatch::create(begin_id, begin_time.into(), end_id, end_time.into(), tx)
                            .await?;
                    batches.push(new_batch);
                }
                Ok::<_, MailContextError>(Some(batches))
            })
            .await?
            .inspect(|batches| {
                tracing::info!(
                    "Created {} batches across {} messages",
                    batches.len(),
                    message_count
                );
            }))
    }

    pub async fn sync_loop<API>(
        &self,
        mut previous_batch: Option<SyncBatch>,
        api: &API,
        stash: &Stash<UserDb>,
        queue: &Queue<UserDb>,
        search_service: Option<&MailSearchService>,
        mut network_status_observer: NetworkStatusObserver,
    ) -> Result<(), MailContextError>
    where
        API: BackwardSyncApi,
    {
        loop {
            match self
                .sync_next(previous_batch.as_ref(), api, stash, queue, search_service)
                .await
            {
                Ok(Some(next_batches)) => {
                    debug_assert!(
                        !next_batches.is_empty(),
                        "sync_next returned Some with an empty Vec"
                    );
                    // Each call yields chunks newest→oldest; the oldest chunk's end_id
                    // is the anchor for the next page.
                    previous_batch = next_batches.into_iter().last();
                }
                Ok(None) => {
                    tracing::info!("Backward Sync Completed");
                    return Ok(());
                }
                Err(MailContextError::Api(e)) if e.is_network_failure() => {
                    tracing::info!("No network, waiting until connection is restored");
                    network_status_observer.wait_until_online().await;
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }
}

impl BackwardSyncDriver for BackwardSync {
    fn sync_loop<API: BackwardSyncApi>(
        &self,
        previous_batch: Option<SyncBatch>,
        api: &API,
        stash: &Stash<UserDb>,
        queue: &Queue<UserDb>,
        search_service: Option<&MailSearchService>,
        network_status_observer: NetworkStatusObserver,
    ) -> impl std::future::Future<Output = Result<(), MailContextError>> + Send {
        BackwardSync::sync_loop(
            self,
            previous_batch,
            api,
            stash,
            queue,
            search_service,
            network_status_observer,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::dependencies::DependencyApi;
    use mail_account_api::protocol::proton::GetAddressResponse;
    use mail_action_queue::queue::TokioTaskSpawner;
    use mail_api::services::proton::prelude::{GetMessagesResponse, MessageId, MessageMetadata};
    use mail_api::services::proton::responses::RunningTasks;
    use mail_common::test_utils::db::new_test_connection_file;
    use mail_core_api::service::{ApiServiceError, ApiServiceResult};
    use mail_core_api::services::proton::{Address as ApiAddress, AddressId};
    use mail_core_common::datatypes::{LabelType, UnixTimestamp};
    use mail_core_common::models::{Address, Label, LabelError};
    use mail_core_common::test_utils::addresses::ApiAddressTestUtils;
    use mail_core_common::test_utils::test_context::test_network_monitor_service_config;
    use mail_network_monitor_service::NetworkMonitorService;
    use mail_stash::stash::{StashError, Tether};

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

    fn messages_response(messages: Vec<MessageMetadata>) -> GetMessagesResponse {
        let total = messages.len() as u64;
        GetMessagesResponse {
            messages,
            tasks_running: RunningTasks::NotKnown,
            stale: false,
            total,
        }
    }

    fn test_message(id: &str, time: u64, address_id: &AddressId) -> MessageMetadata {
        MessageMetadata {
            id: MessageId::from(id),
            time,
            address_id: address_id.clone(),
            ..MessageMetadata::test_default()
        }
    }

    async fn seed_address(tether: &mut Tether, remote_id: &str) -> AddressId {
        let id = AddressId::from(remote_id);
        let api_addr = ApiAddress {
            id: id.clone(),
            ..ApiAddress::test_address()
        };
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                let mut address = Address::from(api_addr);
                address.save(tx).await
            })
            .await
            .unwrap();
        id
    }

    #[tokio::test]
    async fn sync_next_returns_none_when_api_returns_no_messages() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();

        let mut api = MockApi::new();
        api.expect_get_messages()
            .once()
            .returning(|_| Ok(messages_response(vec![])));

        let sync = BackwardSync::new(BackwardSyncParams::default());
        let result = sync
            .sync_next(None, &api, &stash, &queue, None)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn sync_next_propagates_get_messages_error() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();

        let mut api = MockApi::new();
        api.expect_get_messages().once().returning(|_| {
            Err(ApiServiceError::NetworkError(
                "simulated network failure".to_owned(),
            ))
        });

        let sync = BackwardSync::new(BackwardSyncParams::default());
        let result = sync.sync_next(None, &api, &stash, &queue, None).await;

        assert!(
            matches!(result, Err(MailContextError::Api(_))),
            "expected MailContextError::Api, got {result:?}"
        );
    }

    #[tokio::test]
    async fn sync_next_from_scratch_creates_batch_with_first_and_last_message() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();

        let mut tether = stash.connection();
        let address_id = seed_address(&mut tether, "addr-1").await;
        drop(tether);

        let m1 = test_message("msg-newest", 1_000, &address_id);
        let m2 = test_message("msg-middle", 800, &address_id);
        let m3 = test_message("msg-oldest", 500, &address_id);

        let mut api = MockApi::new();
        api.expect_get_messages()
            .once()
            .returning(move |_| Ok(messages_response(vec![m1.clone(), m2.clone(), m3.clone()])));

        let sync = BackwardSync::new(BackwardSyncParams::default());
        let batches = sync
            .sync_next(None, &api, &stash, &queue, None)
            .await
            .unwrap()
            .expect("expected at least one batch");

        assert_eq!(batches.len(), 1, "default params should not chunk");
        let batch = &batches[0];
        assert_eq!(batch.begin_id, MessageId::from("msg-newest"));
        assert_eq!(batch.begin_time.as_u64(), 1_000);
        assert_eq!(batch.end_id, MessageId::from("msg-oldest"));
        assert_eq!(batch.end_time.as_u64(), 500);
    }

    #[tokio::test]
    async fn sync_next_passes_previous_batch_anchor_to_api() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();

        let mut tether = stash.connection();
        let previous = tether
            .write_tx::<_, _, StashError>(async |tx| {
                SyncBatch::create(
                    MessageId::from("prev-begin"),
                    UnixTimestamp::new(2_000),
                    MessageId::from("prev-end"),
                    UnixTimestamp::new(1_500),
                    tx,
                )
                .await
            })
            .await
            .unwrap();
        drop(tether);

        let mut api = MockApi::new();
        api.expect_get_messages()
            .once()
            .withf(|options| {
                options.end_id == Some(MessageId::from("prev-end"))
                    && options.end == Some(1_500)
                    && options.desc == Some(true)
            })
            .returning(move |_| Ok(messages_response(vec![])));

        let sync = BackwardSync::new(BackwardSyncParams::default());
        let result = sync
            .sync_next(Some(&previous), &api, &stash, &queue, None)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn sync_next_drops_duplicate_anchor_message_on_resume() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();

        let mut tether = stash.connection();
        let address_id = seed_address(&mut tether, "addr-1").await;
        let previous = tether
            .write_tx::<_, _, StashError>(async |tx| {
                SyncBatch::create(
                    MessageId::from("prev-begin"),
                    UnixTimestamp::new(2_000),
                    MessageId::from("prev-end"),
                    UnixTimestamp::new(1_500),
                    tx,
                )
                .await
            })
            .await
            .unwrap();
        drop(tether);

        // The API echoes the previous batch's end_id at position 0; sync_next should
        // drop it so the new batch's begin_id is the next message.
        let dup = test_message("prev-end", 1_500, &address_id);
        let m1 = test_message("msg-after-anchor", 1_400, &address_id);
        let m2 = test_message("msg-tail", 1_300, &address_id);

        let mut api = MockApi::new();
        api.expect_get_messages()
            .once()
            .returning(move |_| Ok(messages_response(vec![dup.clone(), m1.clone(), m2.clone()])));

        let sync = BackwardSync::new(BackwardSyncParams::default());
        let batches = sync
            .sync_next(Some(&previous), &api, &stash, &queue, None)
            .await
            .unwrap()
            .expect("expected at least one batch");

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].begin_id, MessageId::from("msg-after-anchor"));
        assert_eq!(batches[0].end_id, MessageId::from("msg-tail"));
    }

    #[tokio::test]
    async fn sync_next_splits_messages_into_chunks_when_chunk_split_set() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();

        let mut tether = stash.connection();
        let address_id = seed_address(&mut tether, "addr-1").await;
        drop(tether);

        // 6 messages with chunk_split=2 → 3 batches of (2,2,2), newest→oldest.
        let messages: Vec<_> = (0..6_u64)
            .map(|i| test_message(&format!("msg-{i}"), 1000 - i * 10, &address_id))
            .collect();

        let mut api = MockApi::new();
        let returned = messages.clone();
        api.expect_get_messages()
            .once()
            .returning(move |_| Ok(messages_response(returned.clone())));

        let sync = BackwardSync::new(BackwardSyncParams {
            page_size: NonZeroUsize::new(10).unwrap(),
            chunk_split: Some(NonZeroUsize::new(2).unwrap()),
        });
        let batches = sync
            .sync_next(None, &api, &stash, &queue, None)
            .await
            .unwrap()
            .expect("expected batches");

        assert_eq!(batches.len(), 3, "6 messages / chunk=2 → 3 batches");
        assert_eq!(batches[0].begin_id, MessageId::from("msg-0"));
        assert_eq!(batches[0].begin_time.as_u64(), 1_000);
        assert_eq!(batches[0].end_id, MessageId::from("msg-1"));
        assert_eq!(batches[0].end_time.as_u64(), 990);
        assert_eq!(batches[1].begin_id, MessageId::from("msg-2"));
        assert_eq!(batches[1].begin_time.as_u64(), 980);
        assert_eq!(batches[1].end_id, MessageId::from("msg-3"));
        assert_eq!(batches[1].end_time.as_u64(), 970);
        assert_eq!(batches[2].begin_id, MessageId::from("msg-4"));
        assert_eq!(batches[2].begin_time.as_u64(), 960);
        assert_eq!(batches[2].end_id, MessageId::from("msg-5"));
        assert_eq!(batches[2].end_time.as_u64(), 950);
    }

    #[tokio::test]
    async fn sync_next_handles_partial_trailing_chunk() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();

        let mut tether = stash.connection();
        let address_id = seed_address(&mut tether, "addr-1").await;
        drop(tether);

        // 5 messages with chunk_split=2 → batches of sizes (2, 2, 1).
        let messages: Vec<_> = (0..5_u64)
            .map(|i| test_message(&format!("msg-{i}"), 1000 - i * 10, &address_id))
            .collect();

        let mut api = MockApi::new();
        let returned = messages.clone();
        api.expect_get_messages()
            .once()
            .returning(move |_| Ok(messages_response(returned.clone())));

        let sync = BackwardSync::new(BackwardSyncParams {
            page_size: NonZeroUsize::new(10).unwrap(),
            chunk_split: Some(NonZeroUsize::new(2).unwrap()),
        });
        let batches = sync
            .sync_next(None, &api, &stash, &queue, None)
            .await
            .unwrap()
            .expect("expected batches");

        assert_eq!(batches.len(), 3);
        // Trailing chunk has a single message; begin and end collapse to the same id.
        assert_eq!(batches[2].begin_id, MessageId::from("msg-4"));
        assert_eq!(batches[2].end_id, MessageId::from("msg-4"));
    }

    #[tokio::test]
    async fn sync_next_emits_single_batch_when_chunk_split_exceeds_message_count() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();

        let mut tether = stash.connection();
        let address_id = seed_address(&mut tether, "addr-1").await;
        drop(tether);

        let m1 = test_message("msg-newest", 1_000, &address_id);
        let m2 = test_message("msg-oldest", 500, &address_id);

        let mut api = MockApi::new();
        api.expect_get_messages()
            .once()
            .returning(move |_| Ok(messages_response(vec![m1.clone(), m2.clone()])));

        let sync = BackwardSync::new(BackwardSyncParams {
            page_size: NonZeroUsize::new(10).unwrap(),
            chunk_split: Some(NonZeroUsize::new(100).unwrap()),
        });
        let batches = sync
            .sync_next(None, &api, &stash, &queue, None)
            .await
            .unwrap()
            .expect("expected a batch");

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].begin_id, MessageId::from("msg-newest"));
        assert_eq!(batches[0].end_id, MessageId::from("msg-oldest"));
    }

    #[tokio::test]
    async fn sync_next_forwards_page_size_to_request_options() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();

        let mut api = MockApi::new();
        api.expect_get_messages()
            .once()
            .withf(|options| options.page_size == 50)
            .returning(|_| Ok(messages_response(vec![])));

        let sync = BackwardSync::new(BackwardSyncParams {
            page_size: NonZeroUsize::new(50).unwrap(),
            chunk_split: None,
        });
        let result = sync
            .sync_next(None, &api, &stash, &queue, None)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn sync_loop_terminates_when_sync_next_returns_none() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();
        let network_service = NetworkMonitorService::new(test_network_monitor_service_config());
        let observer = network_service.network_status_observer();

        let mut api = MockApi::new();
        api.expect_get_messages()
            .once()
            .returning(|_| Ok(messages_response(vec![])));

        let sync = BackwardSync::new(BackwardSyncParams::default());
        sync.sync_loop(None, &api, &stash, &queue, None, observer)
            .await
            .expect("sync_loop should terminate cleanly");
    }

    #[tokio::test]
    async fn sync_loop_anchors_next_call_on_oldest_chunk_of_previous_batches() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();
        let network_service = NetworkMonitorService::new(test_network_monitor_service_config());
        let observer = network_service.network_status_observer();

        let mut tether = stash.connection();
        let address_id = seed_address(&mut tether, "addr-1").await;
        drop(tether);

        // 4 messages, chunked into 2 batches of 2.
        let m0 = test_message("msg-0", 1_000, &address_id);
        let m1 = test_message("msg-1", 900, &address_id);
        let m2 = test_message("msg-2", 800, &address_id);
        let m3 = test_message("msg-3", 700, &address_id);

        let mut api = MockApi::new();
        let mut seq = mockall::Sequence::new();

        // First call: no anchor, returns 4 messages.
        api.expect_get_messages()
            .once()
            .in_sequence(&mut seq)
            .withf(|options| options.end_id.is_none() && options.end.is_none())
            .returning(move |_| {
                Ok(messages_response(vec![
                    m0.clone(),
                    m1.clone(),
                    m2.clone(),
                    m3.clone(),
                ]))
            });

        // Second call: anchored on the OLDEST chunk's tail (msg-3 / t=700), not the
        // newest chunk's. Returns empty so the loop exits.
        api.expect_get_messages()
            .once()
            .in_sequence(&mut seq)
            .withf(|options| {
                options.end_id == Some(MessageId::from("msg-3")) && options.end == Some(700)
            })
            .returning(|_| Ok(messages_response(vec![])));

        let sync = BackwardSync::new(BackwardSyncParams {
            page_size: NonZeroUsize::new(10).unwrap(),
            chunk_split: Some(NonZeroUsize::new(2).unwrap()),
        });
        sync.sync_loop(None, &api, &stash, &queue, None, observer)
            .await
            .expect("sync_loop should terminate cleanly");
    }

    #[tokio::test]
    async fn sync_loop_propagates_non_network_errors() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let queue = Queue::new(stash.clone(), TokioTaskSpawner).await.unwrap();
        let network_service = NetworkMonitorService::new(test_network_monitor_service_config());
        let observer = network_service.network_status_observer();

        let mut api = MockApi::new();
        api.expect_get_messages().once().returning(|_| {
            Err(ApiServiceError::Unauthorized(
                "simulated 401".to_owned(),
                None,
            ))
        });

        let sync = BackwardSync::new(BackwardSyncParams::default());
        let result = sync
            .sync_loop(None, &api, &stash, &queue, None, observer)
            .await;
        assert!(
            matches!(result, Err(MailContextError::Api(_))),
            "expected non-network Api error to propagate, got {result:?}"
        );
    }
}
