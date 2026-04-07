//! Search Index Worker
//!
//! This module implements the background worker that processes search index intents.
//! It runs as a single task and processes intents serially.

use std::sync::Arc;
use std::time::{Duration, Instant};

use mail_api::services::proton::common::MessageId;
use mail_html_transformer::html_to_text_fast;
use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashError, WatcherHandle};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::intent::{LocalMessageId, SearchIndexIntent, SearchOperation};
use crate::service::MailSearchService;
use crate::traits::{BatchPreparedMessage, MIME_TYPE_HTML, MessageDataProvider};

/// Worker-specific error types
///
/// This enum provides type-safe error handling instead of string matching.
#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    /// Message has no remote ID yet - should defer processing
    #[error("Message has no remote ID - should defer")]
    MissingRemoteId,
    /// Other error occurred during processing
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<StashError> for WorkerError {
    fn from(err: StashError) -> Self {
        WorkerError::Other(anyhow::anyhow!("Database error: {}", err))
    }
}

/// Maximum number of retries before giving up on an intent
const MAX_RETRY_COUNT: u64 = 3;

/// Delay after processing an intent (to avoid hammering the CPU)
const PROCESSING_DELAY: Duration = Duration::from_millis(10);

/// Delay when intents are pending to allow them to accumulate before processing
/// This is important because overall indexing process is slow, so we want to batch as many as possible
/// to reduce Load/Save events
///
/// We always wait a small amount when intents are pending to let prefetch workers create more.
/// Progressive delays for larger batches.
const BULK_ACCUMULATION_DELAY_MIN: Duration = Duration::from_millis(2000); // Always wait if any pending
const BULK_ACCUMULATION_DELAY_MEDIUM: Duration = Duration::from_millis(4000); // If >= 20 pending
const BULK_ACCUMULATION_DELAY_LARGE: Duration = Duration::from_millis(10000); // If >= 50 pending

/// Thresholds for bulk accumulation delay
/// Lower thresholds allow longer delays to kick in sooner, helping batches grow toward `MAX_BATCH_SIZE`
const BULK_ACCUMULATION_THRESHOLD_MEDIUM: i64 = 20; // Wait 4s if >= 20 pending
const BULK_ACCUMULATION_THRESHOLD_LARGE: i64 = 50; // Wait 10s if >= 50 pending

/// Delay for deferring intents without remote IDs (1 minute)
/// This prevents messages without remote IDs from blocking the queue
const DEFER_DELAY_SECONDS: i64 = 60;

/// Minimum batch size for processing intents
const MIN_BATCH_SIZE: usize = 50;

/// Maximum batch size for bulk loading mode
///
/// Caps how many documents go into one Foundation Search `commit` via `index_message_bodies_batch`.
/// Lower values trade more Load/Save work for friendlier segment layout (often better query latency
/// after bulk load); raise to speed up indexing on I/O-bound devices.
const MAX_BATCH_SIZE: usize = 1000;

/// The search index worker
///
/// This worker processes search index intents from the database,
/// ensuring serialized access to the search engine.
///
/// Generic over `P` to allow different `MessageDataProvider` implementations.
pub struct SearchIndexWorker<P: MessageDataProvider> {
    mail_stash: Stash<UserDb>,
    search_service: MailSearchService,
    data_provider: Arc<P>,
    /// Watcher handle for receiving notifications when `search_index_intents` table changes
    /// The watcher automatically detects changes after transactions commit, eliminating
    /// race conditions and supporting multi-account scenarios.
    watcher_handle: WatcherHandle,
}

/// Result of preparing a message for indexing
enum PrepareIndexResult {
    /// Message is ready to be indexed with the provided data
    Ready {
        remote_id: MessageId,
        body: String,
        metadata: crate::traits::MessageMetadata,
        content_hash: String,
    },
    /// Message should be deferred (no remote ID yet)
    Defer,
    /// Message should be skipped (local draft or missing data) - intent should remain
    Skip,
    /// Message is a duplicate and intent should be deleted
    SkipDuplicate,
}

impl<P: MessageDataProvider> SearchIndexWorker<P> {
    /// Create a new search index worker
    pub fn new(
        mail_stash: Stash<UserDb>,
        search_service: MailSearchService,
        data_provider: Arc<P>,
        watcher_handle: WatcherHandle,
    ) -> Self {
        Self {
            mail_stash,
            search_service,
            data_provider,
            watcher_handle,
        }
    }

    /// Wait for watcher notification
    ///
    /// Returns `true` if notification received, `false` if watcher closed (worker should exit)
    async fn wait_for_watcher_notification(&self, context: &str) -> bool {
        if let Ok(()) = self.watcher_handle.receiver.recv_async().await {
            debug!(
                "Worker woken up by table watcher notification ({})",
                context
            );
            true
        } else {
            // Watcher closed - database watcher has been closed, exit worker
            error!("Database watcher closed ({}), exiting worker", context);
            false
        }
    }

    /// Run the worker loop
    ///
    /// This method runs indefinitely, processing intents as they arrive.
    ///
    /// Uses database table watcher for event-driven notification: waits for changes
    /// to the `search_index_intents` table. The watcher only fires after transactions
    /// commit, eliminating race conditions. Cleanup runs when the queue is empty.
    pub async fn run(&self) {
        debug!("Search index worker started (table watcher)");

        loop {
            match self.process_batch().await {
                Ok(processed) => {
                    if processed {
                        // Processed intents, check for more immediately
                        // but add a small delay to avoid CPU hammering
                        sleep(PROCESSING_DELAY).await;
                    } else {
                        // No intents pending, check for cleanup then wait for notification
                        if let Err(e) = self.try_cleanup().await {
                            error!("Cleanup failed: {}", e);
                        }

                        // Wait for table change notification
                        // The watcher will notify us immediately when intents are queued (after commit)
                        if !self.wait_for_watcher_notification("normal operation").await {
                            return;
                        }
                    }
                }
                Err(e) => {
                    error!("Worker error: {}", e);
                    // On error, wait for notification before retrying
                    if !self.wait_for_watcher_notification("after error").await {
                        return;
                    }
                }
            }
        }
    }

    /// Process a batch of pending intents
    ///
    /// Returns `true` if intents were processed, `false` if queue was empty.
    ///
    /// Uses adaptive batch sizing:
    /// - Fetches up to `MAX_BATCH_SIZE` when at least that many intents are pending
    /// - Otherwise fetches all available when between `MIN_BATCH_SIZE` and `MAX_BATCH_SIZE`
    /// - Requests `MIN_BATCH_SIZE` when fewer are pending (may receive fewer)
    ///
    /// When intents are pending, waits before fetching to allow more to accumulate:
    /// - 2s if any pending, 4s if ≥20 pending, 10s if ≥50 pending.
    ///
    /// This is important because indexing is slow, so we want to batch as many as possible.
    pub(crate) async fn process_batch(&self) -> Result<bool, StashError> {
        let tether = self.mail_stash.connection().await?;

        // Check how many intents are pending
        let mut pending_count = SearchIndexIntent::pending_count(&tether).await?;

        // Always wait briefly when intents are pending to let more accumulate
        // By waiting, we allow prefetch workers to create more intents while we're about to process
        // Progressive delay: more pending = longer wait to accumulate more
        if pending_count > 0 {
            let delay = if pending_count >= BULK_ACCUMULATION_THRESHOLD_LARGE {
                BULK_ACCUMULATION_DELAY_LARGE
            } else if pending_count >= BULK_ACCUMULATION_THRESHOLD_MEDIUM {
                BULK_ACCUMULATION_DELAY_MEDIUM
            } else {
                BULK_ACCUMULATION_DELAY_MIN
            };

            debug!(
                "Bulk mode: {} intents pending, waiting {}ms to allow accumulation",
                pending_count,
                delay.as_millis()
            );
            sleep(delay).await;

            // Re-check count after delay (more intents may have accumulated)
            let updated_count = SearchIndexIntent::pending_count(&tether).await?;
            if updated_count > pending_count {
                debug!(
                    "Bulk mode: intents accumulated from {} to {} during delay",
                    pending_count, updated_count
                );
                pending_count = updated_count;
            }
        }

        // Use adaptive batch size: fetch as many as available, up to MAX_BATCH_SIZE
        // This allows bulk loading to use large batches when many intents are available
        let batch_size = if pending_count >= i64::try_from(MAX_BATCH_SIZE).unwrap_or(i64::MAX) {
            MAX_BATCH_SIZE
        } else if pending_count >= i64::try_from(MIN_BATCH_SIZE).unwrap_or(i64::MAX) {
            usize::try_from(pending_count).unwrap_or(MIN_BATCH_SIZE)
        } else {
            MIN_BATCH_SIZE
        };

        // Get a batch of intents
        let intents = SearchIndexIntent::get_pending_batch(&tether, batch_size).await?;
        if intents.is_empty() {
            return Ok(false);
        }

        // Log batch information
        if intents.len() == batch_size {
            debug!(
                "Fetched full batch: {} intents ({} pending)",
                intents.len(),
                pending_count
            );
        } else {
            debug!(
                "Fetched partial batch: {} intents ({} pending, requested {})",
                intents.len(),
                pending_count,
                batch_size
            );
        }

        debug!("Processing batch of {} intents", intents.len());

        // Separate intents by operation and filter out max retry ones
        let mut index_intents = Vec::new();
        let mut remove_intents = Vec::new();
        let mut dead_letter_intents = Vec::new();

        for intent in intents {
            if intent.retry_count >= MAX_RETRY_COUNT {
                warn!(
                    "Intent exceeded max retries, discarding: {} for message {}",
                    intent.operation, intent.message_id
                );
                dead_letter_intents.push(intent);
            } else {
                match intent.operation {
                    SearchOperation::Index => index_intents.push(intent),
                    SearchOperation::Remove => remove_intents.push(intent),
                }
            }
        }

        // Delete dead letter intents
        if !dead_letter_intents.is_empty() {
            let mut tether = self.mail_stash.connection().await?;
            tether
                .tx::<_, (), StashError>(async |bond| {
                    for intent in &dead_letter_intents {
                        intent.delete(bond).await?;
                    }
                    Ok(())
                })
                .await?;
        }

        // Process index intents in batch
        if !index_intents.is_empty() {
            self.process_index_batch(index_intents).await?;
        }

        // Process remove intents individually (Foundation Search remove is already batched internally)
        for intent in remove_intents {
            self.process_single_intent(intent).await?;
        }

        Ok(true)
    }

    /// Process a single intent (for remove operations or fallback)
    async fn process_single_intent(&self, intent: SearchIndexIntent) -> Result<(), StashError> {
        debug!(
            "Processing intent: {} for message {}",
            intent.operation, intent.message_id
        );

        // Process the intent
        let result = self.execute_intent(&intent).await;

        // Handle result
        match result {
            Ok(()) => {
                debug!(
                    "Intent succeeded: {} for message {}",
                    intent.operation, intent.message_id
                );

                // Success - delete the intent (content_hash was already saved to separate table in index_message)
                let mut tether = self.mail_stash.connection().await?;
                tether
                    .tx::<_, (), StashError>(async |bond| {
                        intent.delete(bond).await?;
                        debug!(
                            "Deleted intent: {} for message {}",
                            intent.operation, intent.message_id
                        );
                        Ok(())
                    })
                    .await?;

                info!(
                    "Completed: {} for message {}",
                    intent.operation, intent.message_id
                );
            }
            Err(e) => {
                match e {
                    WorkerError::MissingRemoteId => {
                        // Defer instead of marking as failed - prevents queue blocking
                        debug!(
                            "Deferring intent for message {} (no remote ID yet)",
                            intent.message_id
                        );
                        let mut tether = self.mail_stash.connection().await?;
                        tether
                            .tx::<_, (), StashError>(async |bond| {
                                intent.defer(bond, DEFER_DELAY_SECONDS).await
                            })
                            .await?;
                    }
                    WorkerError::Other(err) => {
                        // Other failures - increment retry count
                        warn!(
                            "Failed {} for message {}: {}",
                            intent.operation, intent.message_id, err
                        );

                        let mut intent = intent;
                        let mut tether = self.mail_stash.connection().await?;
                        tether
                            .tx::<_, (), StashError>(async |bond| intent.mark_failed(bond).await)
                            .await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Prepare a message for indexing by fetching all required data and checking conditions
    ///
    /// This centralizes the common logic for:
    /// - Checking for local drafts
    /// - Getting remote ID
    /// - Getting and converting body
    /// - Getting metadata
    /// - Computing content hash
    /// - Checking for duplicates
    async fn prepare_message_for_indexing(
        &self,
        message_id: LocalMessageId,
    ) -> Result<PrepareIndexResult, anyhow::Error> {
        // Check if message is being edited locally - skip indexing incomplete content
        let has_local_draft = self
            .data_provider
            .has_local_draft_metadata(message_id)
            .await?;

        if has_local_draft {
            debug!(
                "Message {:?} is being edited locally, skipping index (will index when sent)",
                message_id
            );
            return Ok(PrepareIndexResult::Skip);
        }

        // Get the remote ID - required for cross-device index compatibility
        let remote_id = self.data_provider.get_remote_id(message_id).await?;

        let Some(remote_id) = remote_id else {
            debug!(
                "Message {} has no remote ID yet, deferring (will retry in {}s)",
                message_id, DEFER_DELAY_SECONDS
            );
            return Ok(PrepareIndexResult::Defer);
        };

        // Get the message body (with MIME type info)
        let body_result = self.data_provider.get_body(message_id).await?;

        let Some((body, is_html)) = body_result else {
            debug!(
                "Message body not found for {:?}, skipping index",
                message_id
            );
            return Ok(PrepareIndexResult::Skip);
        };

        // Convert HTML to text if appropriate (only for text/html MIME type).
        // Use fast strip for indexing (no DOM parse) to keep Android/mobile indexing fast.
        let body_to_index = if is_html {
            html_to_text_fast(&body)
        } else {
            body
        };

        // Get message metadata (subject, from, to, cc, bcc)
        // Metadata is required for indexing - skip if not available
        let metadata = self.data_provider.get_metadata(message_id).await?;

        let Some(metadata) = metadata else {
            debug!(
                "Message metadata not found for {:?}, skipping index",
                message_id
            );
            return Ok(PrepareIndexResult::Skip);
        };

        // Compute content hash for duplicate detection
        let content_hash =
            crate::traits::MessageMetadata::compute_content_hash(&body_to_index, Some(&metadata));

        // Check if content hash matches stored hash (duplicate detection)
        let tether_check = self.mail_stash.connection().await?;
        let should_skip =
            SearchIndexIntent::content_hash_matches(message_id, &content_hash, &tether_check)
                .await
                .unwrap_or(false);

        if should_skip {
            debug!(
                "Skipping indexing: content hash matches for message {} (content unchanged)",
                message_id
            );
            return Ok(PrepareIndexResult::SkipDuplicate);
        }

        Ok(PrepareIndexResult::Ready {
            remote_id,
            body: body_to_index,
            metadata,
            content_hash,
        })
    }

    /// Process a batch-prepared message and return a `PrepareIndexResult`
    fn process_batch_prepared_message(
        &self,
        message_id: LocalMessageId,
        prepared: BatchPreparedMessage,
    ) -> Result<PrepareIndexResult, StashError> {
        // Early checks are now done in SQL query, so these should never trigger
        // But keep them as safety checks
        if prepared.has_local_draft {
            debug!(
                "Message {:?} is being edited locally, skipping index (will index when sent)",
                message_id
            );
            return Ok(PrepareIndexResult::Skip);
        }

        let Some(remote_id) = prepared.remote_id else {
            debug!(
                "Message {} has no remote ID yet, deferring (will retry in {}s)",
                message_id, DEFER_DELAY_SECONDS
            );
            return Ok(PrepareIndexResult::Defer);
        };

        // SQL query already filtered for messages with body and no decryption error
        // So body_raw should always be Some and decryption_error should always be None
        let Some(bytes) = &prepared.body_raw else {
            debug!(
                "Message body not found for {:?}, skipping index",
                message_id
            );
            return Ok(PrepareIndexResult::Skip);
        };

        // Convert body from raw bytes to String
        // Do conversion synchronously (like old MessageBody::load()) - UTF-8 conversion is fast enough
        // The old approach does this synchronously in async context, which works fine for small strings
        let body = String::from_utf8_lossy(bytes).to_string();

        // Determine if HTML based on mime_type
        let is_html = prepared
            .body_mime_type
            .is_some_and(|mt| mt == MIME_TYPE_HTML);

        // Convert HTML to text if appropriate (only for text/html MIME type)
        let body_to_index = if is_html {
            html_to_text_fast(&body)
        } else {
            body
        };

        // Parse message metadata (subject, from, to, cc, bcc) only now that we know message will be indexed
        // Metadata is required for indexing - skip if not available
        let metadata = if let Some((subject, sender_str, to_list_str, cc_list_str, bcc_list_str)) =
            &prepared.metadata_raw
        {
            // Use the data provider's parsing method (implemented in mail-common)
            // Note: This is synchronous (CPU-bound JSON parsing), no await needed
            self.data_provider
                .parse_metadata_raw(
                    subject,
                    sender_str,
                    to_list_str.as_deref(),
                    cc_list_str.as_deref(),
                    bcc_list_str.as_deref(),
                )
                .map_err(|e| StashError::Custom(anyhow::anyhow!("Failed to parse metadata: {e}")))?
        } else {
            None
        };

        let Some(metadata) = metadata else {
            debug!(
                "Message metadata not found for {:?}, skipping index",
                message_id
            );
            return Ok(PrepareIndexResult::Skip);
        };

        // Compute content hash for duplicate detection
        let content_hash =
            crate::traits::MessageMetadata::compute_content_hash(&body_to_index, Some(&metadata));

        // Check if content hash matches stored hash (duplicate detection)
        if let Some(stored_hash) = prepared.stored_content_hash
            && stored_hash == content_hash
        {
            debug!(
                "Skipping indexing: content hash matches for message {} (content unchanged)",
                message_id
            );
            return Ok(PrepareIndexResult::SkipDuplicate);
        }

        Ok(PrepareIndexResult::Ready {
            remote_id,
            body: body_to_index,
            metadata,
            content_hash,
        })
    }

    /// Process a batch of index intents
    #[allow(clippy::too_many_lines)]
    async fn process_index_batch(&self, intents: Vec<SearchIndexIntent>) -> Result<(), StashError> {
        let batch_start = Instant::now();

        // Prepare all documents (get bodies, remote IDs, metadata, etc.)
        let mut messages_to_index: Vec<(
            SearchIndexIntent,
            MessageId,
            String,
            crate::traits::MessageMetadata,
            String, // content_hash
        )> = Vec::new();
        let mut intents_to_defer = Vec::new();
        let mut intents_to_delete = Vec::new();

        let prep_start = Instant::now();
        #[cfg(feature = "foundation_search_index_timing")]
        let stopwatch = crate::indexing_timing::BatchStopwatch::start();

        // Use batch preparation if available (much more efficient)
        // Extract message IDs for batch preparation
        let message_ids: Vec<LocalMessageId> = intents.iter().map(|i| i.message_id).collect();

        let batch_results = self
            .data_provider
            .batch_prepare_messages(&message_ids)
            .await
            .map_err(|e| StashError::Custom(anyhow::anyhow!("Batch preparation failed: {e}")))?;

        if let Some(results) = batch_results
            && !results.is_empty()
            && results.len() == intents.len()
        {
            // Use batch-prepared results
            // Process messages sequentially with yields to match old approach behavior
            // The old approach yielded between each message's database call, so we yield here too
            for (intent, prepared) in intents.into_iter().zip(results.into_iter()) {
                // Yield after each message to allow async runtime to schedule other tasks
                // This matches the old approach where each message had its own async database call
                tokio::task::yield_now().await;

                let result = self.process_batch_prepared_message(intent.message_id, prepared)?;
                match result {
                    PrepareIndexResult::Ready {
                        remote_id,
                        body,
                        metadata,
                        content_hash,
                    } => {
                        messages_to_index.push((intent, remote_id, body, metadata, content_hash));
                    }
                    PrepareIndexResult::Defer => {
                        intents_to_defer.push(intent);
                    }
                    PrepareIndexResult::Skip | PrepareIndexResult::SkipDuplicate => {
                        intents_to_delete.push(intent);
                    }
                }
            }
        } else {
            // Batch preparation not supported (Ok(None)) or wrong size — fall back to individual
            for intent in intents {
                match self
                    .prepare_message_for_indexing(intent.message_id)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to prepare message: {e}"))?
                {
                    PrepareIndexResult::Ready {
                        remote_id,
                        body,
                        metadata,
                        content_hash,
                    } => {
                        messages_to_index.push((intent, remote_id, body, metadata, content_hash));
                    }
                    PrepareIndexResult::Defer => {
                        intents_to_defer.push(intent);
                    }
                    PrepareIndexResult::Skip | PrepareIndexResult::SkipDuplicate => {
                        intents_to_delete.push(intent);
                    }
                }
            }
        }

        // Batch delete intents that were skipped or duplicate
        if !intents_to_delete.is_empty() {
            let to_delete = std::mem::take(&mut intents_to_delete);
            let mut tether = self.mail_stash.connection().await?;
            tether
                .tx::<_, (), StashError>(async |bond| {
                    for intent in &to_delete {
                        intent.delete(bond).await?;
                    }
                    Ok(())
                })
                .await?;
        }

        // Defer intents without remote IDs (don't block the queue)
        if !intents_to_defer.is_empty() {
            let mut tether = self.mail_stash.connection().await?;
            tether
                .tx::<_, (), StashError>(async |bond| {
                    for intent in &intents_to_defer {
                        intent.defer(bond, DEFER_DELAY_SECONDS).await?;
                    }
                    Ok(())
                })
                .await?;
        }

        let prep_elapsed = prep_start.elapsed();
        #[cfg(feature = "foundation_search_index_timing")]
        let stopwatch = stopwatch.record_prep_done();
        info!(
            "   Batch preparation complete: {} messages ready in {:.2}s",
            messages_to_index.len(),
            prep_elapsed.as_secs_f64()
        );

        if messages_to_index.is_empty() {
            return Ok(());
        }

        // Batch index all messages (bodies are already converted from HTML to text if needed)
        // This is the CPU-intensive part (tokenization, indexing)
        let index_start = Instant::now();
        let message_refs: Vec<(&MessageId, &str, &crate::traits::MessageMetadata)> =
            messages_to_index
                .iter()
                .map(|(_, remote_id, body, metadata, _)| (remote_id, body.as_str(), metadata))
                .collect();

        let result = self
            .search_service
            .index_message_bodies_batch(&message_refs)
            .await;

        let index_elapsed = index_start.elapsed();
        #[cfg(feature = "foundation_search_index_timing")]
        let stopwatch = stopwatch.record_index_done();
        #[allow(clippy::cast_precision_loss)]
        let rate = messages_to_index.len() as f64 / index_elapsed.as_secs_f64();
        info!(
            "   Indexing phase complete in {:.2}s ({:.1} messages/s)",
            index_elapsed.as_secs_f64(),
            rate
        );

        // Handle results - delete successful intents, mark failed ones
        match result {
            Ok(()) => {
                // All succeeded - update content_hash and delete intents
                debug!(
                    "Batch index succeeded for {} messages",
                    messages_to_index.len()
                );
                let cleanup_start = Instant::now();
                let mut tether = self.mail_stash.connection().await?;
                tether
                    .tx::<_, (), StashError>(async |bond| {
                        for (intent, _, _, _, content_hash) in &messages_to_index {
                            // Save content hash to separate table before deleting intent
                            // This persists the hash even after intent deletion, enabling
                            // future duplicate detection
                            SearchIndexIntent::save_content_hash(
                                intent.message_id,
                                content_hash,
                                bond,
                            )
                            .await?;
                            intent.delete(bond).await?;
                        }
                        Ok(())
                    })
                    .await?;
                let cleanup_elapsed = cleanup_start.elapsed();
                let batch_elapsed = batch_start.elapsed();
                let batch_size = messages_to_index.len();

                #[cfg(feature = "foundation_search_index_timing")]
                stopwatch
                    .record_cleanup_done()
                    .record_batch_complete(batch_size);

                #[allow(clippy::cast_precision_loss)]
                let rate = messages_to_index.len() as f64 / batch_elapsed.as_secs_f64();
                debug!(
                    "Batch complete: size={} | prep={:.3}s | index={:.3}s | cleanup={:.3}s | total={:.3}s | {:.1} msg/s",
                    batch_size,
                    prep_elapsed.as_secs_f64(),
                    index_elapsed.as_secs_f64(),
                    cleanup_elapsed.as_secs_f64(),
                    batch_elapsed.as_secs_f64(),
                    rate
                );
            }
            Err(e) => {
                // Batch failed - mark all as failed for retry
                warn!(
                    "Batch index failed for {} messages: {}",
                    messages_to_index.len(),
                    e
                );
                let mut tether = self.mail_stash.connection().await?;
                tether
                    .tx::<_, (), StashError>(async |bond| {
                        for (mut intent, _, _, _, _) in messages_to_index {
                            intent.mark_failed(bond).await?;
                        }
                        Ok(())
                    })
                    .await?;
            }
        }

        Ok(())
    }

    /// Execute a search intent
    async fn execute_intent(&self, intent: &SearchIndexIntent) -> Result<(), WorkerError> {
        match intent.operation {
            SearchOperation::Index => self.index_message(intent.message_id).await,
            SearchOperation::Remove => self
                .remove_message(intent.message_id)
                .await
                .map_err(WorkerError::Other),
        }
    }

    /// Index a message body
    ///
    /// Uses the remote `MessageId` for indexing so indices can be merged across devices.
    async fn index_message(&self, message_id: LocalMessageId) -> Result<(), WorkerError> {
        match self
            .prepare_message_for_indexing(message_id)
            .await
            .map_err(|e| WorkerError::Other(anyhow::anyhow!("Failed to prepare message: {e}")))?
        {
            PrepareIndexResult::Ready {
                remote_id,
                body,
                metadata,
                content_hash,
            } => {
                self.search_service
                    .index_message_body(&remote_id, &body, &metadata)
                    .await
                    .map_err(|e| {
                        WorkerError::Other(anyhow::anyhow!("Failed to index message: {e}"))
                    })?;

                // Save content hash to separate table after successful indexing
                // This persists the hash even after intent deletion, enabling future duplicate detection
                let mut tether = self.mail_stash.connection().await?;
                tether
                    .tx::<_, (), StashError>(async |bond| {
                        SearchIndexIntent::save_content_hash(message_id, &content_hash, bond).await
                    })
                    .await?;

                Ok(())
            }
            PrepareIndexResult::Defer => {
                // Return a type-safe error that indicates deferral is needed
                // The caller should defer the intent instead of marking it as failed
                Err(WorkerError::MissingRemoteId)
            }
            PrepareIndexResult::Skip => {
                let mut tether = self.mail_stash.connection().await?;
                tether
                    .tx::<_, (), StashError>(async |bond| {
                        SearchIndexIntent {
                            message_id,
                            operation: SearchOperation::Index,
                            retry_count: 0,
                            created_at: 0,
                        }
                        .delete(bond)
                        .await
                    })
                    .await?;
                Ok(())
            }
            PrepareIndexResult::SkipDuplicate => {
                // Delete intent since content hasn't changed
                let mut tether = self.mail_stash.connection().await?;
                tether
                    .tx::<_, (), StashError>(async |bond| {
                        SearchIndexIntent {
                            message_id,
                            operation: SearchOperation::Index,
                            retry_count: 0,
                            created_at: 0,
                        }
                        .delete(bond)
                        .await
                    })
                    .await?;
                Ok(())
            }
        }
    }

    /// Remove a message from the index
    ///
    /// Uses the remote `MessageId` for consistency with indexing.
    async fn remove_message(&self, message_id: LocalMessageId) -> anyhow::Result<()> {
        // Get the remote ID
        let remote_id = self
            .data_provider
            .get_remote_id(message_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get remote ID: {}", e))?;

        let Some(remote_id) = remote_id else {
            // No remote ID = never synced = never indexed
            debug!(
                "Message {} has no remote ID, nothing to remove from index",
                message_id
            );
            return Ok(());
        };

        self.search_service.remove_message(&remote_id).await?;

        // Delete content hash when message is removed from index
        let mut tether = self.mail_stash.connection().await?;
        tether
            .tx::<_, (), StashError>(async |bond| {
                SearchIndexIntent::delete_content_hash(message_id, bond).await
            })
            .await?;

        Ok(())
    }

    /// Run cleanup when queue is empty
    ///
    /// Cleanup is idempotent - if there's nothing to clean, it returns 0.
    /// This is simpler than tracking a separate "cleanup pending" flag.
    async fn try_cleanup(&self) -> Result<(), StashError> {
        debug!("Running periodic cleanup check");

        match self.search_service.cleanup().await {
            Ok(count) => {
                if count > 0 {
                    info!("Cleanup completed: {} obsolete blobs deleted", count);
                }
            }
            Err(e) => {
                warn!("Cleanup failed: {}", e);
            }
        }

        Ok(())
    }
}
