//! Example: Historic load trial
//!
//! This target is built only with the `foundation_search` feature (`required-features` in `Cargo.toml`).
//! walks the same building blocks the app might use (fetch → queue → concurrent prefetch → indexer) with heavier observability (than test) and with DB export.
//!
//! This example demonstrates how to fetch all messages from the server and queue them
//! for indexing. It uses the Direct Session API approach to paginate
//! through all messages as appears the simpliest option.
//!
//! Usage:
//!   cargo run -p mail-search-perf --example historic_load_trial --features foundation_search -- --username <email> --password <pass> [--email-password <mbp>] [--label-id <id>] [--page-size <size>] [--max-messages <count>] [--concurrency <n>]
//!
//! The example will:
//! 1. Log in and create a MailUserContext
//! 2. Fetch all message metadata pages from the server (using cursor-based pagination)
//! 3. Save messages to the database
//! 4. Queue indexing for messages that already have bodies
//! 5. Queue prefetch actions for messages without bodies (which will trigger indexing when bodies are downloaded)

#[path = "historic_load/core.rs"]
mod historic_load_core;

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use mail_action_queue::queue::{
    NoopOnlineStatusWaiterBuilder, QueueAutoExecutorPool, QueueAutoTerminationPolicy,
    TokioTaskSpawner,
};
use mail_common::{
    actions::PREFETCH_ROLLBACK_ACTION_GROUP, datatypes::SystemLabelId,
    db::offline_migrations::run as migrate_mail_db,
};
use mail_core_api::services::proton::LabelId;
use mail_core_common::db::migrations::migrate_core_db;
use mail_core_common::models::{Label, ModelIdExtension};
use mail_historic_search_load::{
    fetch_all_messages, queue_indexing_and_prefetch, wait_until_prefetch_and_search_index_idle,
};
use mail_search::intent::SearchIndexIntent;
use tempfile::TempDir;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    username: String,
    #[arg(short, long)]
    password: String,
    /// Optional mailbox password (required if account uses separate mailbox password)
    #[clap(long)]
    email_password: Option<String>,
    /// Optional label ID to index (defaults to All Mail)
    #[clap(long)]
    label_id: Option<String>,
    /// Page size for fetching messages (default: 100)
    #[clap(long, default_value = "100")]
    page_size: usize,
    /// Maximum number of messages to process (default: unlimited, processes all messages)
    #[clap(long)]
    max_messages: Option<usize>,
    /// Number of concurrent prefetch workers (default: 4)
    #[clap(long, default_value = "4")]
    concurrency: usize,
    /// Batch size for search index processing (default: 5)
    #[clap(long, default_value = "5")]
    _batch_size: usize,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Configure logging to show only our utility's INFO messages
    // Suppress INFO from other modules (show only WARN/ERROR)
    // Exception: Allow INFO from search worker to see batch completion
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into()) // Default to WARN (suppress INFO)
        .parse_lossy(
            "historic_load_trial=info,\
            mail_search::worker=info,\
            mail_search::watcher=info,",
        );
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .init();

    let Args {
        username,
        password,
        email_password,
        label_id,
        page_size,
        max_messages,
        concurrency,
        _batch_size,
    } = Args::parse();

    // Determine database persistence directory (default to ./historic_load_trial_db if not set)
    let persist_dir = std::env::var("HISTORIC_LOAD_TRIAL_DB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            // Default to a local directory in the current working directory
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("historic_load_trial_db")
        });

    // Clean persistent directory at start to ensure fresh DB for each test run
    historic_load_core::remove_dir_all_if_exists(&persist_dir)?;

    info!(
        "Database will be persisted to: {:?} (after run completes)",
        persist_dir
    );

    // Use TempDir for the database (will be copied to persistent location at the end)
    let tmp_dir = TempDir::new().unwrap();
    info!("TMP DIR: {:?}", tmp_dir.path());

    let ctx = historic_load_core::new_mail_context(&tmp_dir).await?;
    let user_ctx =
        historic_load_core::login_and_user_context(&ctx, username, password, email_password)
            .await?;

    // Ensure migrations have run (they should run automatically, but let's be explicit)
    info!("Ensuring database migrations have run...");
    let stash = user_ctx.user_stash();
    migrate_core_db(stash)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run core migrations: {}", e))?;
    migrate_mail_db(stash)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run mail migrations: {}", e))?;
    info!("Database migrations completed");

    // Resolve label, fetch, and queue work behind a short-lived `tether`. Release it before
    // starting concurrent prefetch executors — otherwise this connection stays checked out and
    // can starve the pool (`Failed to acquire connection in the given time limit`).
    let (total_fetched, indexed_count, prefetch_count, prefetch_broadcast_rx, start_time) = {
        let mut tether = user_ctx
            .user_stash()
            .connection()
            .await
            .map_err(|e| anyhow::anyhow!("stash connection: {}", e))?;

        let (local_label_id, remote_label_id) = if let Some(label_id_str) = &label_id {
            let remote_id = LabelId::from(label_id_str.clone());
            let local_id = Label::remote_id_counterpart(remote_id.clone(), &tether)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to resolve label: {}", e))?
                .ok_or_else(|| anyhow::anyhow!("Label not found"))?;
            (local_id, remote_id)
        } else {
            // Default to All Mail
            let all_mail_id = LabelId::all_mail();
            let local_id = Label::remote_id_counterpart(all_mail_id.clone(), &tether)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to resolve All Mail label: {}", e))?
                .ok_or_else(|| anyhow::anyhow!("All Mail label not found"))?;
            (local_id, all_mail_id)
        };

        info!(
            "Starting bulk indexing for label: {:?} (local: {})",
            remote_label_id, local_label_id
        );

        let start_time = Instant::now();

        if let Some(max) = max_messages {
            info!("Limiting to {} messages", max);
        }
        let total_fetched = fetch_all_messages(&user_ctx, remote_label_id, page_size, max_messages)
            .await
            .map_err(|e| anyhow::anyhow!("fetch_all_messages: {}", e))?;

        info!(
            "Fetched {} messages in {:.2}s",
            total_fetched,
            start_time.elapsed().as_secs_f64()
        );

        let (indexed_count, prefetch_count, prefetch_broadcast_rx) =
            queue_indexing_and_prefetch(&user_ctx, &mut tether)
                .await
                .map_err(|e| anyhow::anyhow!("queue_indexing_and_prefetch: {}", e))?;

        Ok::<_, anyhow::Error>((
            total_fetched,
            indexed_count,
            prefetch_count,
            prefetch_broadcast_rx,
            start_time,
        ))
    }?;

    info!(
        "Queued {} messages for indexing and {} for prefetch in {:.2}s",
        indexed_count,
        prefetch_count,
        start_time.elapsed().as_secs_f64()
    );

    info!("Queued actions:");
    info!("  Fetched {} messages from server", total_fetched);
    info!("  Queued {} messages for indexing", indexed_count);
    info!("  Queued {} messages for prefetch", prefetch_count);

    // Show batching information for search index intents
    {
        const WORKER_BATCH_SIZE: usize = 100; // MAX_BATCH_SIZE in mail-search/src/worker.rs
        if indexed_count > 0 {
            let expected_batches = (indexed_count + WORKER_BATCH_SIZE - 1) / WORKER_BATCH_SIZE; // Ceiling division
            info!(
                "  Search index intents: {} intents will be processed in {} batches (batch size: {})",
                indexed_count, expected_batches, WORKER_BATCH_SIZE
            );
        }
    }

    info!("Processing prefetch and indexing...");

    // Start processing: prefetch actions and indexing intents
    let process_start = Instant::now();

    // Start action queue executor pool to process prefetch actions concurrently
    let action_queue = user_ctx.action_queue();
    let online_waiter = NoopOnlineStatusWaiterBuilder;
    let task_spawner = TokioTaskSpawner;
    let executor_pool = QueueAutoExecutorPool::with_termination_policy(
        &action_queue,
        &PREFETCH_ROLLBACK_ACTION_GROUP,
        NonZeroUsize::new(concurrency).unwrap_or_else(|| {
            warn!("Invalid concurrency value {}, using 1", concurrency);
            NonZeroUsize::new(1).unwrap()
        }),
        &online_waiter,
        false, // start_paused = false
        &task_spawner,
        QueueAutoTerminationPolicy::Empty, // Stop when queue is empty
        tracing::Span::current(),
    );
    info!("Started {} concurrent prefetch workers", concurrency);

    // Note: Search worker is automatically started by MailUserContext::init_search_worker()
    // when Origin::App is used. We don't need to create another worker here.
    {
        let search_service_for_cleanup = user_ctx.search_service().clone();

        // Wait for prefetch and indexing to complete
        wait_until_prefetch_and_search_index_idle(
            &user_ctx,
            prefetch_count,
            indexed_count,
            prefetch_broadcast_rx,
        )
        .await
        .map_err(|e| anyhow::anyhow!("wait_until_prefetch_and_search_index_idle: {}", e))?;

        // Wait until all intents are truly processed (count stays at 0 for multiple checks)
        info!("Verifying all intents are processed...");
        let mut consecutive_zeros = 0;
        let mut last_count = usize::MAX;
        let mut max_wait_attempts = 30; // Max 60 seconds
        loop {
            let intent_count = {
                let tether = user_ctx.user_stash().connection().await?;
                SearchIndexIntent::pending_count(&tether).await? as usize
            };

            if intent_count == 0 {
                consecutive_zeros += 1;
                if consecutive_zeros >= 3 {
                    // Count has been 0 for 3 consecutive checks (6 seconds)
                    info!(
                        "All intents processed (verified {} times)",
                        consecutive_zeros
                    );
                    break;
                }
            } else {
                // Count changed, reset counter
                if intent_count != last_count {
                    warn!("{} intents still pending, waiting...", intent_count);
                    last_count = intent_count;
                }
                consecutive_zeros = 0;
            }

            max_wait_attempts -= 1;
            if max_wait_attempts == 0 {
                warn!(
                    "Timeout waiting for intents to complete. {} intents still pending.",
                    intent_count
                );
                if intent_count > 0 {
                    // Show which intents are still pending
                    let tether = user_ctx.user_stash().connection().await?;
                    let pending_intents: Vec<(i64, String)> = tether
                        .sync_query(|conn| {
                            let mut stmt = conn.prepare("SELECT message_id, operation FROM search_index_intents ORDER BY message_id")?;
                            let rows = stmt.query_map([], |row| {
                                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                            })?;
                            let mut result = Vec::new();
                            for row in rows {
                                result.push(row?);
                            }
                            Ok(result)
                        })
                        .await?;
                    info!("Pending intents: {:?}", pending_intents);
                }
                break;
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        // Note: The worker is automatically started by MailUserContext and will continue running.
        // We don't need to stop it for cleanup - it will handle cleanup when the queue is empty.
        // Wait a bit to ensure any in-flight operations complete
        info!("Waiting for any in-flight operations to complete...");
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Check blob count before cleanup
        let blob_count_before: i64 = {
            let tether = user_ctx.user_stash().connection().await?;
            tether
                .sync_query(|conn| {
                    conn.query_row("SELECT COUNT(*) FROM search_index_blobs", [], |row| {
                        row.get(0)
                    })
                    .map_err(mail_stash::stash::StashError::from)
                })
                .await?
        };
        info!("Blobs before cleanup: {}", blob_count_before);

        // Trigger cleanup manually with retry logic
        // The engine uses RwLock, so we need to retry if it's still busy
        info!("Running cleanup...");
        use mail_search::{SearchError, SearchServiceError};
        let mut cleanup_result: Result<usize, SearchServiceError> =
            Err(SearchServiceError::Cleanup(SearchError::EngineBusy));
        let max_retries = 5;
        let mut retry_delay = Duration::from_millis(500);

        for attempt in 1..=max_retries {
            cleanup_result = match tokio::time::timeout(
                Duration::from_secs(30),
                search_service_for_cleanup.cleanup(),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    warn!(
                        "Cleanup timed out after 30 seconds (attempt {}/{})",
                        attempt, max_retries
                    );
                    if attempt < max_retries {
                        tokio::time::sleep(retry_delay).await;
                        retry_delay *= 2; // Exponential backoff
                        continue;
                    }
                    Ok(0) // Return 0 deleted blobs on timeout
                }
            };

            // Check if cleanup succeeded or if we got EngineBusy
            match &cleanup_result {
                Ok(_) => {
                    // Success!
                    if attempt > 1 {
                        info!("Cleanup succeeded on attempt {}", attempt);
                    }
                    break;
                }
                Err(SearchServiceError::Cleanup(SearchError::EngineBusy)) => {
                    if attempt < max_retries {
                        warn!(
                            "Engine busy, retrying in {:?} (attempt {}/{})",
                            retry_delay, attempt, max_retries
                        );
                        tokio::time::sleep(retry_delay).await;
                        retry_delay *= 2; // Exponential backoff
                        continue;
                    } else {
                        warn!(
                            "Cleanup failed: Engine still busy after {} attempts",
                            max_retries
                        );
                        break;
                    }
                }
                Err(e) => {
                    // Other error, don't retry
                    warn!("Cleanup failed with error: {}", e);
                    break;
                }
            }
        }

        // Check blob count after cleanup
        let blob_count_after: i64 = {
            let tether = user_ctx.user_stash().connection().await?;
            tether
                .sync_query(|conn| {
                    conn.query_row("SELECT COUNT(*) FROM search_index_blobs", [], |row| {
                        row.get(0)
                    })
                    .map_err(mail_stash::stash::StashError::from)
                })
                .await?
        };
        info!("Blobs after cleanup: {}", blob_count_after);

        match cleanup_result {
            Ok(deleted_count) => {
                if deleted_count > 0 {
                    info!(
                        "Cleanup completed: {} obsolete blobs deleted",
                        deleted_count
                    );
                    info!(
                        "Blob count: {} -> {} (expected: {})",
                        blob_count_before,
                        blob_count_after,
                        blob_count_before - deleted_count as i64
                    );
                } else {
                    info!("Cleanup completed: Foundation Search reported no cleanup needed");
                    info!(
                        "Blob count unchanged: {} blobs (all may be active)",
                        blob_count_before
                    );
                }
            }
            Err(e) => {
                warn!("Cleanup failed: {}", e);
            }
        }

        // Final check: wait longer to ensure all transactions are committed
        info!("Final verification before stopping worker...");
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Check intent count multiple times to ensure transactions are committed
        let mut final_intent_check = usize::MAX;
        for i in 0..5 {
            let count = {
                let tether = user_ctx.user_stash().connection().await?;
                SearchIndexIntent::pending_count(&tether).await? as usize
            };

            if count == 0 && final_intent_check == 0 {
                // Count has been 0 for 2 consecutive checks
                info!("All intents confirmed processed (verified {} times)", i + 1);
                break;
            }

            final_intent_check = count;
            if count > 0 {
                warn!("Check {}: {} intents still remain", i + 1, count);
            }

            if i < 4 {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }

        if final_intent_check > 0 {
            warn!(
                "Warning: {} intents still remain after all processing",
                final_intent_check
            );
            // Show which intents are still pending
            let tether = user_ctx.user_stash().connection().await?;
            let pending_intents: Vec<(i64, String)> = tether
                .sync_query(|conn| {
                    let mut stmt = conn.prepare("SELECT message_id, operation FROM search_index_intents ORDER BY message_id")?;
                    let rows = stmt.query_map([], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })?;
                    let mut result = Vec::new();
                    for row in rows {
                        result.push(row?);
                    }
                    Ok(result)
                })
                .await?;
            info!("Remaining intents: {:?}", pending_intents);
        }

        // Wait a bit more to ensure all transactions are fully committed
        info!("Waiting for all transactions to commit...");
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // Terminate executor pool since we've confirmed all actions are complete
    // (wait_until_prefetch_and_search_index_idle already verified the queue is empty)
    info!("Terminating executor pool...");
    executor_pool.terminate();

    let process_elapsed = process_start.elapsed().as_secs_f64();

    info!("Bulk indexing complete!");
    info!("Total time: {:.2}s", start_time.elapsed().as_secs_f64());
    info!("Processing time: {:.2}s", process_elapsed);

    // Print aggregated metrics summary
    // TODO: Add metrics collection once we implement it
    // Metrics will be added incrementally, e.g.:
    // let metrics = user_ctx.search_service().metrics();
    // metrics.print_summary();

    // Final verification of messages processed
    let total_processed = indexed_count + prefetch_count;
    info!("Processing Summary:");
    info!("  Messages fetched from server: {}", total_fetched);
    info!("  Messages indexed (had bodies): {}", indexed_count);
    info!("  Messages prefetched (needed bodies): {}", prefetch_count);
    info!(
        "  Total messages processed: {} (indexed + prefetched)",
        total_processed
    );

    // Show batching summary for search index intents
    {
        const WORKER_BATCH_SIZE: usize = 100; // MAX_BATCH_SIZE in mail-search/src/worker.rs
        if indexed_count > 0 {
            let expected_batches = (indexed_count + WORKER_BATCH_SIZE - 1) / WORKER_BATCH_SIZE; // Ceiling division
            info!(
                "  Search index batching: {} intents processed in {} batches (batch size: {})",
                indexed_count, expected_batches, WORKER_BATCH_SIZE
            );
        }
    }
    if let Some(max) = max_messages {
        if total_fetched == max {
            info!("Successfully processed all {} requested messages", max);
        } else if total_fetched < max {
            warn!(
                "Only {} messages available (requested {})",
                total_fetched, max
            );
        } else {
            warn!(
                "Processed {} messages (requested {}, limit exceeded)",
                total_fetched, max
            );
        }
    } else {
        info!("Processed all available messages");
    }

    // Verify counts match
    if total_processed != total_fetched {
        warn!(
            "Note: Total processed ({}) differs from fetched ({})",
            total_processed, total_fetched
        );
        info!("This is normal if some messages were filtered or already processed");
    }

    // Show database locations
    let user_id = user_ctx.user_id();
    let session_db_path = tmp_dir.path().join("session").join("account.db");
    let user_db_path = tmp_dir.path().join("user").join(format!("{}.db", user_id));

    info!("Database location (temporary):");
    info!("  Session DB: {:?}", session_db_path);
    info!("  User DB: {:?}", user_db_path);
    info!("  Temp directory: {:?}", tmp_dir.path());

    // Always persist the database to the determined location (create directory if needed)
    {
        // Final check of database state before copying
        let user_id_for_copy = user_ctx.user_id();
        {
            info!("Final database state check:");
            let final_intent_count = {
                let tether = user_ctx.user_stash().connection().await?;
                SearchIndexIntent::pending_count(&tether).await? as usize
            };
            let final_blob_count: i64 = {
                let tether = user_ctx.user_stash().connection().await?;
                tether
                    .sync_query(|conn| {
                        conn.query_row("SELECT COUNT(*) FROM search_index_blobs", [], |row| {
                            row.get(0)
                        })
                        .map_err(mail_stash::stash::StashError::from)
                    })
                    .await?
            };
            info!("  Intents: {}", final_intent_count);
            info!("  Blobs: {}", final_blob_count);
        }

        info!("Checkpointing WAL (merging changes into main database)...");
        historic_load_core::persist_mail_databases(&tmp_dir, &persist_dir, user_id_for_copy)
            .await?;

        let dest = persist_dir
            .join("user")
            .join(format!("{}.db", user_id_for_copy));
        info!("You can now inspect the database:");
        info!(
            "sqlite3 {:?} \"SELECT COUNT(*) FROM search_index_intents;\"",
            dest
        );
        info!(
            "sqlite3 {:?} \"SELECT COUNT(*) FROM search_index_blobs;\"",
            dest
        );
    }

    // Keep tmp_dir alive so database persists (for inspection)
    // Uncomment the next line to prevent cleanup:
    let _keep_alive = tmp_dir;

    Ok(())
}
