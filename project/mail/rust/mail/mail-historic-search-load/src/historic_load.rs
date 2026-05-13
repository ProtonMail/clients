//! Historic load helper for bulk indexing messages
//!
//! This module provides functionality to fetch messages from the server and queue them
//! for indexing and prefetch. It is used by the `historic_load_test` and `historic_load_trial`
//! examples in `mail-search-perf`, and can be called from the app via `UniFFI` for on-device perf profiling.
//!
//! ## Public API
//!
//! - [`historic_load_messages`] - High-level convenience function that fetches, queues, and waits
//! - [`fetch_all_messages`] - Fetches messages from server using cursor-based pagination
//! - [`queue_indexing_and_prefetch`] - Queues indexing for messages with bodies and prefetch for those without,
//!   and returns an optional queue broadcast receiver
//!
//! ## Interruption / resume
//!
//! [`fetch_all_messages`] commits one transaction per page; stops leave earlier pages in the DB.
//! To fetch the **next older** slice after a bounded run, pass [`HistoricFetchContinuation`] with the
//! previous batch’s boundary (`oldest_saved_message_time` + `oldest_saved_message_remote_id` from
//! [`FetchAllMessagesSummary`]). The first request then uses the same anchor as an internal “page 2”.

use std::sync::Arc;

use futures::future;
use mail_action_queue::queue::{BroadcastMessage, Queue};
use mail_api::services::proton::{ProtonMail, common::MessageId, requests::GetMessagesOptions};
use mail_common::{
    MailContextError, MailUserContext,
    actions::messages::{
        BATCH_PREFETCH_SIZE, BatchPrefetch, Prefetch, batch_prefetch_can_ingest_bodies,
    },
    datatypes::labels::{ScrollOrderDir, ScrollOrderField},
    datatypes::{LocalMessageId, ReadFilter, SystemLabelId, dependencies::DependencyFetcher},
    models::Message,
};
use mail_core_api::services::proton::LabelId;
use mail_core_common::datatypes::LocalLabelId;
use mail_core_common::models::{Label, ModelIdExtension};
use mail_stash::rusqlite::Connection;
use mail_stash::stash::{StashError, Tether};
use mail_stash::{UserDb, stash::WriteTx};
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info, warn};

use mail_common::search::{MailSearchService, SearchIndexIntent};

/// Maximum time to wait for prefetch and indexing to complete before returning.
/// Prefetch/indexing continue in the background; the app can show "completed" and refresh stats.
const WAIT_FOR_COMPLETION_TIMEOUT: Duration = Duration::from_secs(300 * 60); // 300 minutes

const SQL_MESSAGES_WITH_BODY_PENDING_INDEX: &str = r"
    SELECT DISTINCT mb.message_id
    FROM raw_message_body mb
    LEFT JOIN search_index_intents sii
      ON mb.message_id = sii.message_id
      AND sii.operation = 'index'
    WHERE mb.decryption_error IS NULL
      AND LENGTH(mb.body) > 0
      AND sii.message_id IS NULL
";

const SQL_MESSAGES_WITHOUT_BODY: &str = r"
    SELECT m.local_id FROM messages m
    LEFT JOIN raw_message_body mb ON m.local_id = mb.message_id
    WHERE m.remote_id IS NOT NULL
    AND m.deleted = 0
    AND mb.message_id IS NULL
";

const INDEX_INTENT_QUEUE_BATCH: usize = 1000;
const PREFETCH_ACTION_JOIN_CHUNK: usize = 100;
const PREFETCH_QUEUE_ERRORS_STOP: usize = 10;
const PREFETCH_LOG_COUNT: usize = 1000;
const PREFETCH_BACKPRESSURE_CHECK_EVERY_CHUNKS: usize = 5;
const PREFETCH_BACKPRESSURE_HIGH_WATERMARK: usize = 5000;
const PREFETCH_BACKPRESSURE_LOW_WATERMARK: usize = 2000;
const PREFETCH_BACKPRESSURE_SLEEP: Duration = Duration::from_secs(2);

/// Resume token: next [`fetch_all_messages`] call starts with the same cursor as after this message
/// (anchor time + id), returning the next **older** page(s) in descending-time order.
#[derive(Debug, Clone)]
pub struct HistoricFetchContinuation {
    pub anchor_time: u64,
    pub anchor_message_id: MessageId,
}

/// Summary of [`fetch_all_messages`]
#[derive(Debug, Clone)]
pub struct FetchAllMessagesSummary {
    pub messages_saved: usize,
    /// Unix time in seconds: minimum message time among rows saved in this call.
    /// With newest-first fetch order, this is the **oldest** message in the batch (the boundary of the youngest-`max` window).
    pub oldest_saved_message_time: Option<u64>,
    /// Remote message id at [`Self::oldest_saved_message_time`] (tie-break by id when times match).
    pub oldest_saved_message_remote_id: Option<String>,
}

/// Result of a historic load operation
#[derive(Debug, Clone)]
pub struct HistoricLoadResult {
    /// Number of messages fetched from server
    pub messages_fetched: usize,
    /// Number of messages queued for indexing (includes both immediate and prefetched messages)
    /// This represents the total number of messages that will be indexed
    pub messages_indexed: usize,
    /// Number of messages queued for prefetch (needed bodies)
    pub messages_prefetched: usize,
    /// Unix time in seconds of the oldest message in the fetched batch (see [`FetchAllMessagesSummary::oldest_saved_message_time`]).
    pub oldest_saved_message_time: Option<u64>,
    /// Remote id of that oldest message; pass with the time as [`HistoricFetchContinuation`] to continue older.
    pub oldest_saved_message_remote_id: Option<String>,
}

/// Fetch messages from the server and queue them for indexing/prefetch
///
/// This function:
/// 1. Fetches messages from the server using cursor-based pagination
/// 2. Saves messages to the database
/// 3. Queues indexing for messages that already have bodies
/// 4. Queues prefetch actions for messages without bodies
///
/// # Arguments
/// * `user_ctx` - The mail user context
/// * `label_id` - Optional label ID to fetch from (defaults to All Mail)
/// * `max_messages` - Optional maximum number of messages to process (**youngest / newest first** by server time)
/// * `page_size` - Page size for fetching (default: 100)
/// * `continuation` - Optional anchor (`HistoricFetchContinuation`) to fetch the next **older** slice instead of from newest
///
/// # Returns
/// A `HistoricLoadResult` with counts of fetched, indexed, and prefetched messages, plus the oldest
/// message time in the fetched batch (when `max_messages` is set, that batch is the youngest *N* messages).
pub async fn historic_load_messages(
    user_ctx: &Arc<MailUserContext>,
    label_id: Option<LabelId>,
    max_messages: Option<usize>,
    page_size: Option<usize>,
    continuation: Option<HistoricFetchContinuation>,
) -> Result<HistoricLoadResult, MailContextError> {
    let page_size = page_size.unwrap_or(100);
    let stash = user_ctx.user_stash().clone();
    let mut tether = stash.connection();

    let (remote_label_id, not_found): (LabelId, &'static str) = match label_id {
        Some(id) => (id, "Label not found"),
        None => (LabelId::all_mail(), "All Mail label not found"),
    };
    let local_label_id: LocalLabelId =
        Label::remote_id_counterpart(remote_label_id.clone(), &tether)
            .await
            .map_err(|e| {
                MailContextError::Other(anyhow::anyhow!("Failed to resolve label: {}", e))
            })?
            .ok_or_else(|| MailContextError::Other(anyhow::anyhow!(not_found)))?;

    info!(
        "Starting historic load for label: {:?} (local: {})",
        remote_label_id, local_label_id
    );

    if let Some(max) = max_messages {
        info!("Limiting to {} messages", max);
    }

    // Fetch messages from server (newest first; see `ScrollOrderDir::Desc` + `ScrollOrderField::Time` in `fetch_all_messages`)
    let fetch_summary = fetch_all_messages(
        user_ctx,
        remote_label_id,
        page_size,
        max_messages,
        continuation,
    )
    .await
    .map_err(|e| MailContextError::Other(anyhow::anyhow!("Failed to fetch messages: {}", e)))?;

    let messages_fetched = fetch_summary.messages_saved;
    let oldest_saved_message_time = fetch_summary.oldest_saved_message_time;
    let oldest_saved_message_remote_id = fetch_summary.oldest_saved_message_remote_id.clone();

    info!("Fetched {} messages from server", messages_fetched);
    if let Some(t) = oldest_saved_message_time {
        info!("Oldest message time in fetched batch (unix secs): {}", t);
    }
    if let Some(id) = &oldest_saved_message_remote_id {
        info!("Oldest message remote id in fetched batch: {}", id);
    }

    // Queue indexing and prefetch
    let (messages_indexed_immediate, messages_prefetched, prefetch_broadcast_rx) =
        queue_indexing_and_prefetch(user_ctx, &mut tether)
            .await
            .map_err(|e| {
                MailContextError::Other(anyhow::anyhow!("Failed to queue actions: {}", e))
            })?;

    info!(
        "Queued {} messages for indexing (already had bodies) and {} for prefetch",
        messages_indexed_immediate, messages_prefetched
    );

    // Total messages that will be indexed = immediate + prefetched (prefetched messages will be indexed after bodies are fetched)
    let messages_indexed = messages_indexed_immediate + messages_prefetched;

    // Wait for prefetch and indexing to complete (same as historic_load_trial.rs)
    // This ensures we get accurate timing measurements
    if messages_indexed > 0 || messages_prefetched > 0 {
        info!("Waiting for prefetch and indexing to complete...");
        wait_until_prefetch_and_search_index_idle(
            user_ctx,
            messages_prefetched,
            messages_indexed,
            prefetch_broadcast_rx,
        )
        .await
        .map_err(|e| {
            MailContextError::Other(anyhow::anyhow!("Failed to wait for completion: {}", e))
        })?;
    }

    Ok(HistoricLoadResult {
        messages_fetched,
        messages_indexed,
        messages_prefetched,
        oldest_saved_message_time,
        oldest_saved_message_remote_id,
    })
}

/// Fetch messages from the server using cursor-based pagination
///
/// This function fetches message metadata from the Proton server and saves them to the database.
/// It uses cursor-based pagination to efficiently handle large mailboxes.
///
/// # Interruption
///
/// Completed pages remain committed; errors or process exit stop the loop. Without
/// [`HistoricFetchContinuation`], the next call starts from the newest page again (metadata upserts
/// by remote id). With `continuation`, the first request uses the stored anchor so the next **older**
/// slice is fetched.
///
/// # Arguments
/// * `user_ctx` - The mail user context
/// * `remote_label_id` - Remote label ID to fetch messages from
/// * `page_size` - Number of messages to fetch per page
/// * `max_messages` - Optional maximum number of messages to fetch
/// * `continuation` - Optional anchor from a previous run’s batch boundary (oldest time + remote id in that batch)
///
/// # Returns
/// Count of messages saved and the oldest **message** (unix time + remote id) in that saved set.
///
/// Messages are requested in **descending time order** (newest first). With `max_messages`, the
/// saved set is the youngest *N* messages in the label; `oldest_saved_message_time` is the time of
/// the oldest among those *N* (the in-mailbox lower bound of the window).
#[allow(clippy::too_many_lines)] // cursor pagination + persist loop
pub async fn fetch_all_messages(
    user_ctx: &Arc<MailUserContext>,
    remote_label_id: LabelId,
    page_size: usize,
    max_messages: Option<usize>,
    continuation: Option<HistoricFetchContinuation>,
) -> Result<FetchAllMessagesSummary, anyhow::Error> {
    let session = user_ctx.session();
    let stash = user_ctx.user_stash().clone();
    let unread = ReadFilter::All;
    let order_dir = ScrollOrderDir::Desc;
    let order_field = ScrollOrderField::Time;

    let mut total_pages = if continuation.is_some() { 1 } else { 0 };
    let mut last_message_id = continuation.as_ref().map(|c| c.anchor_message_id.clone());
    let mut last_message_time = continuation.as_ref().map(|c| c.anchor_time);
    let mut total_messages_saved = 0;
    // Running minimum (time, id) with deterministic tie-break on id.
    let mut oldest_saved: Option<(u64, MessageId)> = None;

    if let Some(c) = &continuation {
        if c.anchor_message_id.as_str().is_empty() {
            return Err(anyhow::anyhow!(
                "Historic fetch continuation: anchor_message_id must not be empty"
            ));
        }
        info!(
            "Resuming historic fetch from anchor time={} id={}",
            c.anchor_time, c.anchor_message_id
        );
    }

    let mut page_requests: u32 = 0;
    loop {
        page_requests = page_requests.saturating_add(1);
        let mut opts = GetMessagesOptions {
            label_id: Some(vec![remote_label_id.clone()]),
            page_size: if total_pages == 0 {
                page_size as u64
            } else {
                (page_size as u64) + 1 // +1 to detect end
            },
            unread: unread.into(),
            desc: order_dir.as_api_desc(),
            sort: order_field.as_api_sort(),
            ..Default::default()
        };
        if total_pages == 0 {
            info!("Fetching first page (page_size={})", page_size);
        } else {
            let anchor_time = last_message_time.unwrap();
            let anchor_id = last_message_id.as_ref().unwrap();
            info!(
                "Fetching next page {} (anchor_id={:?}, anchor_time={})",
                total_pages + 1,
                anchor_id,
                anchor_time
            );
            opts.anchor = Some(anchor_time);
            opts.anchor_id = Some(anchor_id.clone());
        }

        let response = ProtonMail::get_messages(session, opts).await?;

        if response.messages.is_empty() {
            info!("No more messages to fetch");
            break;
        }

        // Handle anchor message (first message in response is usually the anchor from previous page)
        let mut messages_to_save = response.messages;
        if total_pages > 0
            && !messages_to_save.is_empty()
            && let Some(last_id) = &last_message_id
        {
            if messages_to_save[0].id == *last_id {
                messages_to_save.remove(0);
            } else if messages_to_save.len() > page_size {
                messages_to_save.pop();
            }
        }

        if messages_to_save.is_empty() {
            info!("No new messages in this page");
            break;
        }

        // Trim messages if we're approaching the max_messages limit
        if let Some(max) = max_messages {
            if total_messages_saved >= max {
                break;
            }
            let remaining = max - total_messages_saved;
            if messages_to_save.len() > remaining {
                info!(
                    "Trimming page to {} messages (limit: {}, already saved: {})",
                    remaining, max, total_messages_saved
                );
                messages_to_save.truncate(remaining);
            }
        }

        // Save messages to database
        info!("Saving {} messages to database", messages_to_save.len());

        // Resolve dependencies first
        let mut dependency_fetcher = DependencyFetcher::new();
        for message in &messages_to_save {
            dependency_fetcher
                .check_api_message_metadata(message, &stash.connection())
                .await?;
        }
        let mut tether = stash.connection();
        dependency_fetcher
            .fetch_and_store(session, &mut tether)
            .await?;

        // Save messages within a transaction using public API
        let saved_messages = tether
            .quiet_write_tx(async |tx| {
                Message::create_or_update_messages_from_metadata_vec(
                    messages_to_save,
                    None, // No event action
                    tx,
                )
                .await
                .map_err(|e| {
                    MailContextError::Other(anyhow::anyhow!("Failed to save messages: {}", e))
                })
            })
            .await?;

        total_messages_saved += saved_messages.len();
        for m in &saved_messages {
            let Some(rid) = m.remote_id.clone() else {
                continue;
            };
            let t = m.time.as_u64();
            let replace = match &oldest_saved {
                None => true,
                Some((ot, oid)) if t < *ot => true,
                Some((ot, oid)) if t == *ot && rid < *oid => true,
                _ => false,
            };
            if replace {
                oldest_saved = Some((t, rid));
            }
        }
        info!(
            "Saved {} messages (total: {})",
            saved_messages.len(),
            total_messages_saved
        );

        // Check if we've reached the max_messages limit
        if let Some(max) = max_messages
            && total_messages_saved >= max
        {
            info!(
                "Reached max_messages limit ({} >= {})",
                total_messages_saved, max
            );
            break;
        }

        // Update cursor for next page
        if let Some(last) = saved_messages.last() {
            last_message_id.clone_from(&last.remote_id);
            last_message_time = Some(last.time.as_u64());
        } else {
            break;
        }

        total_pages += 1;

        // Check if we've reached the end
        if saved_messages.len() < page_size {
            info!(
                "Reached end of messages (got {} < {})",
                saved_messages.len(),
                page_size
            );
            break;
        }
    }

    info!(
        "Finished fetching: {} page request(s), {} total messages",
        page_requests, total_messages_saved
    );
    let (oldest_saved_message_time, oldest_saved_message_remote_id) = match oldest_saved {
        None => (None, None),
        Some((t, id)) => (Some(t), Some(id.to_string())),
    };
    Ok(FetchAllMessagesSummary {
        messages_saved: total_messages_saved,
        oldest_saved_message_time,
        oldest_saved_message_remote_id,
    })
}

/// Queue indexing for messages with bodies and prefetch for messages without bodies
///
/// This function:
/// - Finds messages with bodies that need indexing and queues them
/// - Finds messages without bodies that need prefetch and queues them
pub async fn queue_indexing_and_prefetch(
    user_ctx: &Arc<MailUserContext>,
    tether: &mut Tether,
) -> Result<
    (
        usize,
        usize,
        Option<tokio::sync::broadcast::Receiver<BroadcastMessage>>,
    ),
    anyhow::Error,
> {
    let _search_service = user_ctx.search_service();

    let messages_with_bodies: Vec<LocalMessageId> = tether
        .sync_query(|conn| load_local_message_ids(conn, SQL_MESSAGES_WITH_BODY_PENDING_INDEX))
        .await?;
    info!(
        "Found {} messages with bodies that need indexing",
        messages_with_bodies.len()
    );
    let indexed_count = queue_search_index_batches(tether, &messages_with_bodies).await?;

    let messages_without_bodies: Vec<LocalMessageId> = tether
        .sync_query(|conn| load_local_message_ids(conn, SQL_MESSAGES_WITHOUT_BODY))
        .await?;
    info!(
        "Found {} messages without bodies that need prefetch",
        messages_without_bodies.len()
    );

    debug!(
        "About to queue prefetch for {} messages",
        messages_without_bodies.len()
    );

    let prefetch_broadcast_rx = (!messages_without_bodies.is_empty())
        .then(|| user_ctx.action_queue().new_broadcast_receiver());

    let (prefetch_count, prefetch_errors) = queue_prefetch_for_missing_bodies(
        user_ctx.as_ref(),
        &messages_without_bodies,
        batch_prefetch_can_ingest_bodies(),
    )
    .await;

    if prefetch_errors > 0 {
        warn!(
            "Failed to queue {} prefetch actions (successfully queued: {}) - messages will be fetched on-demand",
            prefetch_errors, prefetch_count
        );
    } else if prefetch_count > 0 {
        info!("Queued all {} prefetch actions", prefetch_count);
    }

    Ok((indexed_count, prefetch_count, prefetch_broadcast_rx))
}

async fn prefetch_actions_pending_count(
    action_queue: &Queue<UserDb>,
) -> Result<usize, MailContextError> {
    let n = action_queue
        .typed_actions_count::<Prefetch>()
        .await
        .map_err(MailContextError::from)?
        + action_queue
            .typed_actions_count::<BatchPrefetch>()
            .await
            .map_err(MailContextError::from)?;
    Ok(usize::try_from(n).unwrap_or(0))
}

fn load_local_message_ids(conn: &Connection, sql: &str) -> Result<Vec<LocalMessageId>, StashError> {
    let mut stmt = conn.prepare(sql).map_err(StashError::from)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, LocalMessageId>(0))
        .map_err(StashError::from)?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row.map_err(StashError::from)?);
    }
    Ok(ids)
}

async fn queue_search_index_batches(
    tether: &mut Tether,
    message_ids: &[LocalMessageId],
) -> Result<usize, anyhow::Error> {
    let total = message_ids.len();
    let mut queued = 0;
    for chunk in message_ids.chunks(INDEX_INTENT_QUEUE_BATCH) {
        let chunk_ids: Vec<u64> = chunk.iter().map(LocalMessageId::as_u64).collect();
        tether
            .write_tx(async |bond: &WriteTx<'_, UserDb>| {
                MailSearchService::queue_index_batch(&chunk_ids, bond).await
            })
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to queue indexing batch (chunk size: {}): {}",
                    chunk_ids.len(),
                    e
                )
            })?;
        queued += chunk.len();
        if queued % 5000 == 0 {
            info!("Queued {} messages for indexing...", queued);
        }
    }
    info!("Queued all {total} messages for indexing");
    Ok(total)
}

async fn queue_prefetch_for_missing_bodies(
    user_ctx: &MailUserContext,
    message_ids: &[LocalMessageId],
    use_batch: bool,
) -> (usize, usize) {
    let queue = user_ctx.action_queue();
    let mut prefetch_count = 0_usize;
    let mut prefetch_errors = 0_usize;
    let mut should_stop = false;

    if use_batch {
        for (chunk_idx, chunk) in message_ids.chunks(BATCH_PREFETCH_SIZE).enumerate() {
            if should_stop {
                warn!("Stopping batch prefetch queueing at chunk {chunk_idx}");
                break;
            }
            let ids: Vec<LocalMessageId> = chunk.to_vec();
            match queue.queue_action(BatchPrefetch::new(ids)).await {
                Ok(_) => {
                    prefetch_count += chunk.len();
                    if chunk_idx < 3 {
                        debug!(
                            "BatchPrefetch chunk {chunk_idx} ({} messages) queued",
                            chunk.len()
                        );
                    }
                }
                Err(e) => {
                    prefetch_errors += chunk.len();
                    if prefetch_errors <= 3 {
                        error!("Failed to queue BatchPrefetch: {e}");
                    }
                    if prefetch_errors >= PREFETCH_QUEUE_ERRORS_STOP {
                        warn!("Stopping batch prefetch queueing after errors");
                        should_stop = true;
                    }
                }
            }
            if prefetch_count > 0 && prefetch_count.is_multiple_of(PREFETCH_LOG_COUNT) {
                info!("Queued {prefetch_count} messages for batch prefetch...");
            }
            if (chunk_idx + 1).is_multiple_of(PREFETCH_BACKPRESSURE_CHECK_EVERY_CHUNKS) {
                maybe_apply_prefetch_backpressure(user_ctx).await;
            }
        }
        debug!(
            "BatchPrefetch queueing complete: {prefetch_count} messages in {} batches, {prefetch_errors} errors",
            message_ids.len().div_ceil(BATCH_PREFETCH_SIZE)
        );
    } else {
        for (chunk_idx, chunk) in message_ids.chunks(PREFETCH_ACTION_JOIN_CHUNK).enumerate() {
            if should_stop {
                warn!("Stopping prefetch queueing at chunk {chunk_idx}");
                break;
            }
            let mut futures = Vec::with_capacity(chunk.len());
            for message_id in chunk {
                futures.push(queue.queue_action(Prefetch::new(*message_id)));
            }
            for (idx, result) in future::join_all(futures).await.into_iter().enumerate() {
                match result {
                    Ok(_) => {
                        prefetch_count += 1;
                        if idx < 3 {
                            debug!("Prefetch action {idx} succeeded");
                        }
                    }
                    Err(e) => {
                        prefetch_errors += 1;
                        if prefetch_errors <= 3 {
                            error!(
                                "Failed to queue prefetch action (error {prefetch_errors}): {e}"
                            );
                        }
                        if prefetch_errors >= PREFETCH_QUEUE_ERRORS_STOP {
                            warn!("Stopping prefetch queueing after {prefetch_errors} errors");
                            should_stop = true;
                            break;
                        }
                    }
                }
            }
            if prefetch_count > 0 && prefetch_count.is_multiple_of(PREFETCH_LOG_COUNT) {
                info!("Queued {prefetch_count} prefetch actions...");
            }
            if (chunk_idx + 1).is_multiple_of(PREFETCH_BACKPRESSURE_CHECK_EVERY_CHUNKS) {
                maybe_apply_prefetch_backpressure(user_ctx).await;
            }
        }
        debug!("Prefetch queueing complete: {prefetch_count} succeeded, {prefetch_errors} errors");
    }

    (prefetch_count, prefetch_errors)
}

async fn maybe_apply_prefetch_backpressure(user_ctx: &MailUserContext) {
    let Some(mut pending) = load_pending_index_intent_count(user_ctx).await else {
        return;
    };
    if pending < PREFETCH_BACKPRESSURE_HIGH_WATERMARK {
        return;
    }

    info!(
        "Applying prefetch backpressure: pending index intents={} (high watermark={})",
        pending, PREFETCH_BACKPRESSURE_HIGH_WATERMARK
    );
    while pending > PREFETCH_BACKPRESSURE_LOW_WATERMARK {
        tokio::time::sleep(PREFETCH_BACKPRESSURE_SLEEP).await;
        match load_pending_index_intent_count(user_ctx).await {
            Some(next_pending) => {
                pending = next_pending;
            }
            None => break,
        }
    }
    info!(
        "Backpressure released: pending index intents={} (low watermark={})",
        pending, PREFETCH_BACKPRESSURE_LOW_WATERMARK
    );
}

async fn load_pending_index_intent_count(user_ctx: &MailUserContext) -> Option<usize> {
    let tether = user_ctx.user_stash().connection();
    match SearchIndexIntent::pending_count(&tether).await {
        Ok(count) => Some(usize::try_from(count).unwrap_or(0)),
        Err(e) => {
            warn!("Failed to read pending index intents: {e}");
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn log_historic_idle_progress_if_due(
    last_report: &mut Instant,
    last_prefetch_count: &mut usize,
    last_index_count: &mut usize,
    prefetch_pending: usize,
    index_pending: usize,
    peak_prefetch_pending: usize,
    peak_index_pending: usize,
) {
    if last_report.elapsed().as_secs() < 5 {
        return;
    }

    let prefetch_processed_since_last = last_prefetch_count.saturating_sub(prefetch_pending);
    let index_processed_since_last = last_index_count.saturating_sub(index_pending);

    let elapsed_since_last = last_report.elapsed().as_secs_f64();
    let prefetch_rate = if elapsed_since_last > 0.0 && prefetch_processed_since_last > 0 {
        #[allow(clippy::cast_precision_loss)]
        {
            prefetch_processed_since_last as f64 / elapsed_since_last
        }
    } else {
        0.0
    };
    let index_rate = if elapsed_since_last > 0.0 && index_processed_since_last > 0 {
        #[allow(clippy::cast_precision_loss)]
        {
            index_processed_since_last as f64 / elapsed_since_last
        }
    } else {
        0.0
    };

    let prefetch_pct = if peak_prefetch_pending > 0 {
        let processed = peak_prefetch_pending.saturating_sub(prefetch_pending);
        #[allow(clippy::cast_precision_loss)]
        {
            (processed as f64 / peak_prefetch_pending as f64) * 100.0
        }
    } else {
        100.0
    };
    let index_pct = if peak_index_pending > 0 {
        let processed = peak_index_pending.saturating_sub(index_pending);
        #[allow(clippy::cast_precision_loss)]
        {
            (processed as f64 / peak_index_pending as f64) * 100.0
        }
    } else {
        100.0
    };

    info!(
        "Progress: {} prefetch remaining ({:.1}% done, {:.1}/s), {} indexing remaining ({:.1}% done, {:.1}/s)",
        prefetch_pending, prefetch_pct, prefetch_rate, index_pending, index_pct, index_rate
    );

    *last_prefetch_count = prefetch_pending;
    *last_index_count = index_pending;
    *last_report = Instant::now();
}

/// Wait until prefetch actions and [`SearchIndexIntent`] backlog are cleared (or timeout).
pub async fn wait_until_prefetch_and_search_index_idle(
    user_ctx: &Arc<MailUserContext>,
    initial_prefetch_count: usize,
    initial_indexed_count: usize,
    mut prefetch_completion_rx: Option<tokio::sync::broadcast::Receiver<BroadcastMessage>>,
) -> Result<(), MailContextError> {
    let stash = user_ctx.user_stash().clone();

    let mut last_prefetch_count = initial_prefetch_count;
    let mut last_index_count = initial_indexed_count;
    let mut last_report = Instant::now();
    let wait_start = Instant::now();

    // Track peak counts to handle cases where new intents are created during processing
    let tether_initial = stash.connection();
    let initial_index_pending =
        usize::try_from(SearchIndexIntent::pending_count(&tether_initial).await?).unwrap_or(0);
    drop(tether_initial);
    let initial_prefetch_pending = prefetch_actions_pending_count(user_ctx.action_queue()).await?;

    let mut peak_index_pending = initial_index_pending.max(initial_indexed_count).max(1);
    let mut peak_prefetch_pending = initial_prefetch_pending.max(initial_prefetch_count).max(1);

    loop {
        let prefetch_pending = prefetch_actions_pending_count(user_ctx.action_queue()).await?;

        let tether = stash.connection();
        let index_pending =
            usize::try_from(SearchIndexIntent::pending_count(&tether).await?).unwrap_or(0);
        drop(tether);

        // Update peak counts (new intents can be created as prefetch completes)
        peak_index_pending = peak_index_pending.max(index_pending);
        peak_prefetch_pending = peak_prefetch_pending.max(prefetch_pending);

        log_historic_idle_progress_if_due(
            &mut last_report,
            &mut last_prefetch_count,
            &mut last_index_count,
            prefetch_pending,
            index_pending,
            peak_prefetch_pending,
            peak_index_pending,
        );

        // Check if both are complete
        if prefetch_pending == 0 && index_pending == 0 {
            info!("All prefetch and indexing complete!");
            break;
        }

        // Stop waiting after timeout so the app can complete; workers continue in background
        if wait_start.elapsed() >= WAIT_FOR_COMPLETION_TIMEOUT {
            warn!(
                "Stopping wait after {:.0}s: {} prefetch and {} indexing still pending (will continue in background)",
                WAIT_FOR_COMPLETION_TIMEOUT.as_secs_f64(),
                prefetch_pending,
                index_pending
            );
            break;
        }

        let mut broadcast_closed = false;
        match &mut prefetch_completion_rx {
            Some(rx) => {
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_secs(2)) => {}
                    recv = rx.recv() => {
                        match recv {
                            Ok(_) | Err(RecvError::Lagged(_)) => {}
                            Err(RecvError::Closed) => broadcast_closed = true,
                        }
                    }
                }
            }
            None => {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
        if broadcast_closed {
            prefetch_completion_rx = None;
        }
    }

    Ok(())
}
