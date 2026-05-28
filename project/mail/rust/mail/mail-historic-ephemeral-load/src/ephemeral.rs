use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use crate::EPHEMERAL_BODY_SUBCHUNK_SIZE;
use futures::stream::{self, StreamExt};
use mail_action_queue::action::ActionGroup;
use mail_action_queue::queue::{Queue, QueuedError};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api::services::proton::ProtonMail;
use mail_api::services::proton::common::MessageId;
use mail_api::services::proton::prelude::MessageMetadata as ApiMessageMetadata;
use mail_common::datatypes::EncryptedMessageBody;
use mail_common::datatypes::dependencies::DependencyFetcher;
use mail_common::models::{Message, MessageBodyMetadata};
use mail_common::{AppError, MailContextError, MailUserContext};
use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::LabelId;
use mail_core_api::session::Session;
use mail_crypto_inbox::message::{DecryptableMessage as _, DecryptedBody};
use mail_crypto_inbox::proton_crypto;
use mail_crypto_inbox::proton_crypto_account::keys::AddressKeySelector;
use mail_html_transformer::html_to_text_fast;
use mail_search::{MessageMetadata, PreparedIndexCommit, save_blobs_in_write_tx};
use mail_stash::UserDb;
use mail_stash::stash::Stash;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::checkpoint::{EphemeralPageCheckpointWrite, IndexingBatchProgressWrite};
use crate::ephemeral_timing::{EphemeralTimingCollector, EphemeralTimingStats};
use crate::error::EphemeralHistoricLoadError;

#[derive(Debug, Clone)]
pub struct EphemeralHistoricLoadResult {
    pub messages_fetched: usize,
    pub messages_metadata_saved: usize,
    pub messages_indexed: usize,
    pub messages_skipped_missing_body: usize,
    /// All Mail `total` from the first metadata response in this batch (`None`
    /// if the batch made no metadata call or the API returned zero).
    pub mailbox_messages_total: Option<u64>,
    pub oldest_message_time: Option<u64>,
    pub oldest_message_remote_id: Option<String>,
    pub timing: EphemeralTimingStats,
}

/// Ephemeral historic load (single batch): bodies → Foundation Search.
///
/// Processes the pre-fetched batch of metadata produced by the
/// [`mail_common::historic_mailbox_walker::HistoricMailboxWalker`]. Bodies are
/// fetched/decrypted/indexed in sub-chunks of [`EPHEMERAL_BODY_SUBCHUNK_SIZE`]
/// with incremental SQLite persist (checkpoint unchanged) so a retryable
/// network failure mid-batch retains already-indexed messages. After every
/// sub-chunk completes, the All Mail checkpoint and cumulative batch progress
/// are persisted in one transaction. Message metadata is saved for indexed
/// messages only (no bodies, index intents, or prefetch queue), atomically
/// with Foundation Search blobs per sub-chunk persist. When nothing was
/// indexed in the batch, the checkpoint row is left unchanged.
///
/// `cancel` is observed at cooperative yield points: around each sub-chunk
/// body fetch and between per-message decrypt iterations. A signal while a
/// sub-chunk is in flight aborts cleanly with
/// [`EphemeralHistoricLoadError::Cancelled`] *before* any SQLite write happens
/// for that sub-chunk; sub-chunks that already persisted keep their index
/// blobs and leave the checkpoint unchanged until the batch completes.
pub async fn ephemeral_index_batch(
    user_ctx: &Arc<MailUserContext>,
    concurrent_body_fetches: usize,
    messages: Vec<ApiMessageMetadata>,
    mailbox_messages_total: Option<u64>,
    cancel: CancellationToken,
) -> Result<EphemeralHistoricLoadResult, EphemeralHistoricLoadError> {
    let mut timing = EphemeralTimingCollector::default();
    let search_service = user_ctx.search_service();
    let session = user_ctx.session();

    let total_fetched = messages.len();
    info!(
        "Ephemeral historic batch: fetched={} fetch/decrypt bodies + Foundation Search (concurrency={})",
        total_fetched, concurrent_body_fetches
    );

    let mut total_metadata_saved = 0usize;
    let mut total_indexed = 0usize;
    let mut total_skipped_missing_body = 0usize;
    let mut batch_oldest_indexed: Option<(u64, MessageId)> = None;

    let batch_start = Instant::now();

    {
        let mut metadata_save_elapsed = std::time::Duration::ZERO;
        let mut index_elapsed = std::time::Duration::ZERO;

        for subchunk in messages.chunks(EPHEMERAL_BODY_SUBCHUNK_SIZE) {
            if cancel.is_cancelled() {
                return Err(EphemeralHistoricLoadError::Cancelled);
            }

            sync_missing_addresses_for_messages(user_ctx, session, subchunk).await?;

            let message_ids: Vec<_> = subchunk.iter().map(|m| m.id.clone()).collect();
            let session_clone = session.clone();
            let body_fetch_future = stream::iter(message_ids)
                .map(|mid| {
                    let s = session_clone.clone();
                    let key = mid.to_string();
                    async move {
                        (
                            key,
                            ProtonMail::get_message(&s, mid).await.map(|r| r.message),
                        )
                    }
                })
                .buffer_unordered(concurrent_body_fetches)
                .collect::<Vec<_>>();

            let mut fetched_bodies: HashMap<String, Result<_, _>> = tokio::select! {
                biased;
                () = cancel.cancelled() => return Err(EphemeralHistoricLoadError::Cancelled),
                results = body_fetch_future => results.into_iter().collect(),
            };

            if let Some((first_id, first_err, total)) =
                take_first_retryable_body_error(&mut fetched_bodies)
            {
                tracing::warn!(
                    "ephemeral historic load: {total} body fetch(es) failed with retryable network errors \
                     (first: {first_id} - {first_err})"
                );
                return Err(EphemeralHistoricLoadError::RetryableApi(first_err));
            }

            let pgp = proton_crypto::new_pgp_provider();
            let mut subchunk_docs: Vec<(MessageId, String, MessageMetadata)> =
                Vec::with_capacity(subchunk.len());

            for meta_msg in subchunk {
                if cancel.is_cancelled() {
                    return Err(EphemeralHistoricLoadError::Cancelled);
                }
                let Some(body_result) = fetched_bodies.get(meta_msg.id.as_str()) else {
                    tracing::warn!("Body fetch result missing for {}", meta_msg.id);
                    total_skipped_missing_body += 1;
                    continue;
                };
                let api_msg = match body_result {
                    Ok(m) => m.clone(),
                    Err(e) => {
                        tracing::warn!("Failed to fetch body for {}: {e}", meta_msg.id);
                        total_skipped_missing_body += 1;
                        continue;
                    }
                };

                let remote_id = meta_msg.id.clone();
                let address_id = api_msg.metadata.address_id.clone();

                let encrypted = EncryptedMessageBody {
                    encrypted_body: api_msg.body.body,
                    metadata: MessageBodyMetadata {
                        remote_message_id: Some(remote_id.clone()),
                        mime_type: api_msg.body.mime_type.into(),
                        ..Default::default()
                    },
                };

                let tether = user_ctx.user_stash().connection();
                let address_keys = match user_ctx
                    .crypto_key_service()
                    .load_with_tether(user_ctx.user_context(), &tether)
                    .address_keys(&pgp, &address_id)
                    .await
                    .map(AddressKeySelector::into_raw_keys)
                {
                    Ok(keys) => keys,
                    Err(e) => {
                        tracing::warn!("Key loading failed for {}: {e}", remote_id);
                        total_skipped_missing_body += 1;
                        continue;
                    }
                };
                drop(tether);

                let decrypt_start = Instant::now();
                let raw_decrypted = match encrypted.decrypt(&pgp, &address_keys) {
                    Ok(raw) => raw,
                    Err(e) => {
                        timing.record_decrypt(decrypt_start.elapsed());
                        tracing::warn!("Decrypt failed for {}: {e}", remote_id);
                        total_skipped_missing_body += 1;
                        continue;
                    }
                };

                let decrypted_body = match raw_decrypted.processed_body() {
                    Ok(body) => body,
                    Err(e) => {
                        timing.record_decrypt(decrypt_start.elapsed());
                        tracing::warn!("Body processing failed for {}: {e}", remote_id);
                        total_skipped_missing_body += 1;
                        continue;
                    }
                };
                timing.record_decrypt(decrypt_start.elapsed());

                let body_text = match &decrypted_body {
                    DecryptedBody::Plain(text) => {
                        let strip_start = Instant::now();
                        let stripped = html_to_text_fast(text);
                        timing.record_html_strip(strip_start.elapsed());
                        stripped
                    }
                    DecryptedBody::Mime(_) => {
                        let strip_start = Instant::now();
                        let stripped = html_to_text_fast(decrypted_body.body());
                        timing.record_html_strip(strip_start.elapsed());
                        stripped
                    }
                };

                push_doc(
                    &mut subchunk_docs,
                    &mut batch_oldest_indexed,
                    meta_msg,
                    remote_id,
                    body_text,
                );
            }

            if subchunk_docs.is_empty() {
                continue;
            }

            let indexed_ids: HashSet<MessageId> =
                subchunk_docs.iter().map(|(id, _, _)| id.clone()).collect();
            let indexed_api_messages: Vec<ApiMessageMetadata> = subchunk
                .iter()
                .filter(|m| indexed_ids.contains(&m.id))
                .cloned()
                .collect();

            let metadata_prep_start = Instant::now();
            let metadata_to_save = tokio::select! {
                biased;
                () = cancel.cancelled() => return Err(EphemeralHistoricLoadError::Cancelled),
                result = prepare_indexed_messages_metadata(
                    user_ctx,
                    session,
                    &indexed_api_messages,
                ) => result?,
            };
            let metadata_prep_elapsed = metadata_prep_start.elapsed();

            let refs = subchunk_docs
                .iter()
                .map(|(rid, body, meta)| (rid, body.as_str(), meta))
                .collect::<Vec<_>>();

            let index_start = Instant::now();
            let prepared = search_service
                .prepare_index_message_bodies_batch(&refs)
                .await
                .map_err(EphemeralHistoricLoadError::from_search_prepare)?;
            let prepare_elapsed = index_start.elapsed();

            let indexed_this_subchunk = refs.len();
            let persist_start = Instant::now();
            let saved = persist_ephemeral_page(
                user_ctx.user_stash(),
                user_ctx.action_queue(),
                search_service,
                prepared,
                EphemeralPageCheckpointWrite::Unchanged,
                metadata_to_save,
                None,
            )
            .await?;
            let persist_elapsed = persist_start.elapsed();

            metadata_save_elapsed += metadata_prep_elapsed + persist_elapsed;
            index_elapsed += prepare_elapsed + persist_elapsed;
            timing.record_metadata_save(metadata_prep_elapsed + persist_elapsed, saved);
            timing.record_index_only(prepare_elapsed + persist_elapsed, indexed_this_subchunk);
            total_metadata_saved += saved;
            total_indexed += indexed_this_subchunk;
        }

        // Walker passes us the whole batch in one shot, so checkpoint +
        // cumulative batch progress are written atomically once all
        // sub-chunks are persisted. (Pre-walker code advanced the checkpoint
        // once per API page within the rare 2-page batch; that intra-batch
        // granularity is gone in the walker model.)
        let batch_progress = IndexingBatchProgressWrite::from_batch_totals(
            total_fetched,
            total_indexed,
            total_skipped_missing_body,
            mailbox_messages_total,
        );

        let persist_start = Instant::now();
        if total_indexed > 0 {
            debug_assert!(
                batch_oldest_indexed.is_some(),
                "non-zero total_indexed implies batch oldest anchor"
            );
            let checkpoint =
                EphemeralPageCheckpointWrite::from_indexed_page(batch_oldest_indexed.clone());
            persist_page_checkpoint_only(user_ctx.user_stash(), checkpoint, Some(batch_progress))
                .await?;
        } else {
            persist_batch_progress_only(user_ctx.user_stash(), batch_progress).await?;
        }
        metadata_save_elapsed += persist_start.elapsed();
        index_elapsed += persist_start.elapsed();

        info!(
            "Ephemeral batch: fetched={} metadata_saved={} indexed={} skipped={} | metadata_save={:.1}ms index={:.1}ms total={:.1}ms",
            total_fetched,
            total_metadata_saved,
            total_indexed,
            total_skipped_missing_body,
            metadata_save_elapsed.as_secs_f64() * 1000.0,
            index_elapsed.as_secs_f64() * 1000.0,
            batch_start.elapsed().as_secs_f64() * 1000.0,
        );
    }

    let (oldest_message_time, oldest_message_remote_id) = match &batch_oldest_indexed {
        Some((t, id)) => (Some(*t), Some(id.to_string())),
        None => (None, None),
    };

    Ok(EphemeralHistoricLoadResult {
        messages_fetched: total_fetched,
        messages_metadata_saved: total_metadata_saved,
        messages_indexed: total_indexed,
        messages_skipped_missing_body: total_skipped_missing_body,
        mailbox_messages_total,
        oldest_message_time,
        oldest_message_remote_id,
        timing: timing.snapshot(),
    })
}

/// Resolves label/contact dependencies for indexed messages (network I/O, outside SQLite tx).
async fn prepare_indexed_messages_metadata(
    user_ctx: &MailUserContext,
    session: &Session,
    api_messages: &[ApiMessageMetadata],
) -> Result<Vec<ApiMessageMetadata>, EphemeralHistoricLoadError> {
    if api_messages.is_empty() {
        return Ok(Vec::new());
    }

    let stash = user_ctx.user_stash().clone();
    let mut dependency_fetcher = DependencyFetcher::new();
    for message in api_messages {
        dependency_fetcher
            .check_api_message_metadata(message, &stash.connection())
            .await?;
    }

    let mut tether = stash.connection();
    let unresolved_label_ids = dependency_fetcher
        .fetch_and_store(session, &mut tether)
        .await?;

    let mut metadata = api_messages.to_vec();
    prune_unresolved_labels_from_api_metadata(&mut metadata, &unresolved_label_ids);
    Ok(metadata)
}

/// Persist cumulative batch progress in its own transaction (no indexed page).
async fn persist_batch_progress_only(
    mail_stash: &Stash<UserDb>,
    progress: IndexingBatchProgressWrite,
) -> Result<(), EphemeralHistoricLoadError> {
    mail_stash
        .connection()
        .write_tx::<_, (), EphemeralHistoricLoadError>(async |tx| {
            progress
                .persist_in_write_tx(tx)
                .await
                .map_err(EphemeralHistoricLoadError::from_indexing_state)
        })
        .await
}

/// Advance the All Mail checkpoint (and optional batch progress) after all sub-chunks on a page.
pub(crate) async fn persist_page_checkpoint_only(
    mail_stash: &Stash<UserDb>,
    checkpoint: EphemeralPageCheckpointWrite,
    batch_progress: Option<IndexingBatchProgressWrite>,
) -> Result<(), EphemeralHistoricLoadError> {
    mail_stash
        .connection()
        .write_tx::<_, (), EphemeralHistoricLoadError>(async |tx| {
            checkpoint
                .persist_in_write_tx(tx)
                .await
                .map_err(EphemeralHistoricLoadError::from_indexing_state)?;
            if let Some(progress) = batch_progress {
                progress
                    .persist_in_write_tx(tx)
                    .await
                    .map_err(EphemeralHistoricLoadError::from_indexing_state)?;
            }
            Ok(())
        })
        .await
}

/// Fetch and store label/address dependencies for a metadata sub-chunk (network I/O, outside tx).
async fn sync_missing_addresses_for_messages(
    user_ctx: &MailUserContext,
    session: &Session,
    messages: &[ApiMessageMetadata],
) -> Result<(), EphemeralHistoricLoadError> {
    if messages.is_empty() {
        return Ok(());
    }

    let stash = user_ctx.user_stash();
    let mut fetcher = DependencyFetcher::new();
    let tether = stash.connection();
    for message in messages {
        fetcher.check_api_message_metadata(message, &tether).await?;
    }
    drop(tether);

    let mut tether = stash.connection();
    fetcher.fetch_and_store(session, &mut tether).await?;
    Ok(())
}

/// One ACID page: index blobs + checkpoint + message metadata in a single `write_tx`.
///
/// When `batch_progress` is `Some`, cumulative counters (and optional mailbox
/// total) are updated in the same transaction as the checkpoint.
pub(crate) async fn persist_ephemeral_page(
    mail_stash: &Stash<UserDb>,
    action_queue: &Queue<UserDb>,
    search_service: &mail_search::MailSearchService,
    prepared: PreparedIndexCommit,
    checkpoint: EphemeralPageCheckpointWrite,
    metadata: Vec<ApiMessageMetadata>,
    batch_progress: Option<IndexingBatchProgressWrite>,
) -> Result<usize, EphemeralHistoricLoadError> {
    let cleanup_needed = prepared.cleanup_needed;
    let save_operations = prepared.save_operations;

    let saved = mail_stash
        .connection()
        .write_tx::<_, _, EphemeralHistoricLoadError>(async |tx| {
            save_blobs_in_write_tx(tx, save_operations)
                .await
                .map_err(EphemeralHistoricLoadError::page_persist)?;
            checkpoint
                .persist_in_write_tx(tx)
                .await
                .map_err(EphemeralHistoricLoadError::from_indexing_state)?;
            if let Some(progress) = batch_progress {
                progress
                    .persist_in_write_tx(tx)
                    .await
                    .map_err(EphemeralHistoricLoadError::from_indexing_state)?;
            }
            let messages = Message::create_or_update_messages_from_metadata_vec(metadata, None, tx)
                .await
                .map_err(|e: AppError| {
                    EphemeralHistoricLoadError::MetadataPrepare(MailContextError::App(e))
                })?;

            let mut rebase_change_set = RebaseChangeSet::default();
            for message in &messages {
                if let Some(local_id) = message.local_id {
                    rebase_change_set.add(local_id);
                }
            }

            action_queue
                .rebase_in(ActionGroup::default(), &rebase_change_set, tx)
                .await
                .map_err(|e: QueuedError| {
                    EphemeralHistoricLoadError::MetadataPrepare(MailContextError::QueuedAction(e))
                })?;

            Ok(messages)
        })
        .await?;

    if cleanup_needed && let Err(e) = search_service.cleanup().await {
        tracing::warn!("Automatic cleanup after ephemeral page persist failed: {e}");
    }

    Ok(saved.len())
}

/// Drop label ids that could not be fetched/stored (same as [`Message::prune_unresolved_labels`]).
fn prune_unresolved_labels_from_api_metadata(
    messages: &mut [ApiMessageMetadata],
    unresolved_label_ids: &HashSet<LabelId>,
) {
    if unresolved_label_ids.is_empty() {
        return;
    }
    for message in messages {
        message
            .label_ids
            .retain(|label_id| !unresolved_label_ids.contains(label_id));
    }
}

fn push_doc(
    page_docs: &mut Vec<(MessageId, String, MessageMetadata)>,
    oldest_saved: &mut Option<(u64, MessageId)>,
    message: &mail_api::services::proton::prelude::MessageMetadata,
    remote_id: MessageId,
    body: String,
) {
    let metadata = MessageMetadata {
        subject: message.subject.clone(),
        from: message.sender.address.as_clear_text_str().to_owned(),
        to: message
            .to_list
            .iter()
            .map(|r| r.address.as_clear_text_str())
            .collect::<Vec<_>>()
            .join(","),
        cc: message
            .cc_list
            .iter()
            .map(|r| r.address.as_clear_text_str())
            .collect::<Vec<_>>()
            .join(","),
        bcc: message
            .bcc_list
            .iter()
            .map(|r| r.address.as_clear_text_str())
            .collect::<Vec<_>>()
            .join(","),
    };

    let t = message.time;
    let replace = match oldest_saved {
        None => true,
        Some((ot, _)) if t < *ot => true,
        Some((ot, oid)) if t == *ot && remote_id < *oid => true,
        _ => false,
    };
    if replace {
        *oldest_saved = Some((t, remote_id.clone()));
    }

    page_docs.push((remote_id, body, metadata));
}

/// Find the first body-fetch result whose error is a transient network
/// failure, remove it from the map, and return it alongside the total
/// number of network-failure results in the same page.
///
/// Returns `None` when no body-fetch result has
/// [`ApiServiceError::is_network_failure`] set.
///
/// Extracted as a free function so the classification can be unit-tested
/// without standing up the full ephemeral pipeline (which requires a live
/// `MailUserContext` and Proton API session).
fn take_first_retryable_body_error<T>(
    fetched_bodies: &mut HashMap<String, Result<T, ApiServiceError>>,
) -> Option<(String, ApiServiceError, usize)> {
    let total = fetched_bodies
        .values()
        .filter(|result| matches!(result, Err(e) if e.is_network_failure()))
        .count();

    if total == 0 {
        return None;
    }

    let first_id = fetched_bodies.iter().find_map(|(id, result)| {
        matches!(result, Err(e) if e.is_network_failure()).then(|| id.clone())
    })?;
    // `unwrap()` chains are safe: we just observed the entry above and
    // confirmed it is `Err(_)` with a network-failure classification.
    let first_err = fetched_bodies
        .remove(&first_id)
        .expect("entry must exist; we just iterated it")
        .err()
        .expect("entry must be Err; we filtered on Err above");
    Some((first_id, first_err, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata_with_labels(message_id: &str, label_ids: &[&str]) -> ApiMessageMetadata {
        ApiMessageMetadata {
            id: MessageId::from(message_id),
            label_ids: label_ids.iter().map(|id| LabelId::from(*id)).collect(),
            ..ApiMessageMetadata::test_default()
        }
    }

    #[test]
    fn prune_unresolved_labels_empty_set_is_noop() {
        let mut messages = vec![metadata_with_labels("m1", &["5", "0"])];
        prune_unresolved_labels_from_api_metadata(&mut messages, &HashSet::new());
        assert_eq!(
            messages[0].label_ids,
            vec![LabelId::from("5"), LabelId::from("0")]
        );
    }

    #[test]
    fn prune_unresolved_labels_removes_only_unresolved() {
        let unresolved = HashSet::from([LabelId::from("99")]);
        let mut messages = vec![metadata_with_labels("m1", &["5", "99", "0"])];
        prune_unresolved_labels_from_api_metadata(&mut messages, &unresolved);
        assert_eq!(
            messages[0].label_ids,
            vec![LabelId::from("5"), LabelId::from("0")]
        );
    }

    #[test]
    fn prune_unresolved_labels_applies_per_message() {
        let unresolved = HashSet::from([LabelId::from("custom")]);
        let mut messages = vec![
            metadata_with_labels("m1", &["5", "custom"]),
            metadata_with_labels("m2", &["0", "custom"]),
        ];
        prune_unresolved_labels_from_api_metadata(&mut messages, &unresolved);
        assert_eq!(messages[0].label_ids, vec![LabelId::from("5")]);
        assert_eq!(messages[1].label_ids, vec![LabelId::from("0")]);
    }

    fn api_metadata(
        message_id: &str,
        time: u64,
    ) -> mail_api::services::proton::prelude::MessageMetadata {
        mail_api::services::proton::prelude::MessageMetadata {
            id: MessageId::from(message_id),
            time,
            ..ApiMessageMetadata::test_default()
        }
    }

    #[test]
    fn push_doc_tracks_oldest_by_time() {
        let mut page_docs = Vec::new();
        let mut oldest = None;
        push_doc(
            &mut page_docs,
            &mut oldest,
            &api_metadata("newer", 2_000),
            MessageId::from("newer"),
            "body".to_owned(),
        );
        push_doc(
            &mut page_docs,
            &mut oldest,
            &api_metadata("older", 1_000),
            MessageId::from("older"),
            "body".to_owned(),
        );
        assert_eq!(page_docs.len(), 2);
        let (t, id) = oldest.expect("oldest anchor");
        assert_eq!(t, 1_000);
        assert_eq!(id, MessageId::from("older"));
    }

    #[test]
    fn push_doc_tie_breaks_equal_time_by_message_id() {
        let mut page_docs = Vec::new();
        let mut oldest = None;
        let time = 1_500;
        push_doc(
            &mut page_docs,
            &mut oldest,
            &api_metadata("msg-b", time),
            MessageId::from("msg-b"),
            "body".to_owned(),
        );
        push_doc(
            &mut page_docs,
            &mut oldest,
            &api_metadata("msg-a", time),
            MessageId::from("msg-a"),
            "body".to_owned(),
        );
        let (_, id) = oldest.expect("oldest anchor");
        assert_eq!(id, MessageId::from("msg-a"));
    }

    fn ok_body() -> Result<(), ApiServiceError> {
        Ok(())
    }

    fn err_network() -> Result<(), ApiServiceError> {
        Err(ApiServiceError::NetworkError("offline".into()))
    }

    fn err_timeout() -> Result<(), ApiServiceError> {
        Err(ApiServiceError::Timeout("slow".into()))
    }

    fn err_unauthorized() -> Result<(), ApiServiceError> {
        Err(ApiServiceError::Unauthorized("expired".into(), None))
    }

    #[test]
    fn take_first_retryable_returns_none_when_all_bodies_succeed() {
        let mut bodies: HashMap<String, Result<(), ApiServiceError>> =
            HashMap::from([("m1".into(), ok_body()), ("m2".into(), ok_body())]);
        assert!(take_first_retryable_body_error(&mut bodies).is_none());
        assert_eq!(bodies.len(), 2);
    }

    #[test]
    fn take_first_retryable_returns_none_for_only_non_network_errors() {
        let mut bodies: HashMap<String, Result<(), ApiServiceError>> =
            HashMap::from([("m1".into(), err_unauthorized()), ("m2".into(), ok_body())]);
        assert!(take_first_retryable_body_error(&mut bodies).is_none());
        // Non-network failures must remain in the map so the caller can
        // count them as per-message skips.
        assert_eq!(bodies.len(), 2);
    }

    #[test]
    fn take_first_retryable_surfaces_network_error_and_counts_it() {
        let mut bodies: HashMap<String, Result<(), ApiServiceError>> =
            HashMap::from([("m1".into(), err_network()), ("m2".into(), ok_body())]);

        let (id, err, total) =
            take_first_retryable_body_error(&mut bodies).expect("network failure should surface");
        assert_eq!(id, "m1");
        assert!(matches!(err, ApiServiceError::NetworkError(_)));
        assert_eq!(total, 1);
        // The surfaced entry is removed; remaining entries are untouched.
        assert!(!bodies.contains_key("m1"));
        assert!(bodies.contains_key("m2"));
    }

    #[test]
    fn take_first_retryable_counts_all_network_errors_but_returns_one() {
        let mut bodies: HashMap<String, Result<(), ApiServiceError>> = HashMap::from([
            ("m1".into(), err_network()),
            ("m2".into(), err_timeout()),
            ("m3".into(), err_unauthorized()),
            ("m4".into(), ok_body()),
        ]);

        let (id, err, total) =
            take_first_retryable_body_error(&mut bodies).expect("network failure should surface");
        assert_eq!(total, 2, "both network and timeout should count");
        assert!(matches!(
            err,
            ApiServiceError::NetworkError(_) | ApiServiceError::Timeout(_)
        ));
        // The returned id corresponds to one of the two retryable entries.
        assert!(id == "m1" || id == "m2");
        // Exactly one retryable entry has been taken out; the other plus
        // the non-network and Ok results remain in the map.
        assert_eq!(bodies.len(), 3);
        assert!(bodies.contains_key("m3"));
        assert!(bodies.contains_key("m4"));
    }
}
