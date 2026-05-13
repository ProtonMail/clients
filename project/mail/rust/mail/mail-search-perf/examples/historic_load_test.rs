//! Minimal test wrapper for historic_load_messages
//!
//! stress-tests the packaged historic load API and perf/timing/fixtures
//!
//! This example provides a simple way to test the historic_load functionality
//! in the Rust environment before calling it from the iOS app via FFI.
//!
//! ## Feature flags (`--features` on **`mail-search-perf`**)
//!
//! Recommended for perf / fixtures / full timing output:
//! ```text
//! foundation_search,foundation_search_lab_harness,foundation_search_index_timing
//! ```
//!
//! - **`foundation_search`** — Required (`[[example]] required-features`). Historic load + Foundation Search;
//!   on this crate it **also enables** **`foundation_search_lab_harness`** (see `Cargo.toml`).
//! - **`foundation_search_lab_harness`** — Redundant **on this crate** if `foundation_search` is already
//!   enabled, but include it in the triplet for clarity or to match **`mail-common`** / copy-pasted commands.
//! - **`foundation_search_index_timing`** — Turns on **indexing** timing **reset + printed stats** in this
//!   binary (`#[cfg]` around `mail_search::indexing_timing::*`). Prefetch/fixture output does not need this flag.
//!
//! Usage (from `project/mail/rust`), minimal smoke (no indexing timing section):
//!   cargo run -p mail-search-perf --example historic_load_test --features foundation_search -- --username <email> --password <pass> ...
//!
//! Full perf / timing / fixtures (use the triplet above):
//!   cargo run -p mail-search-perf --example historic_load_test --features "foundation_search,foundation_search_lab_harness,foundation_search_index_timing" -- ...
//!
//! **Prefetch timing** counts per-message body fetch (normal HTTP + decrypt in `Message::fetch_message_body_impl`)
//! and cache hits — not `BatchPrefetch` or fixture substitution. Those paths store bodies without
//! `sync_message_and_body`, so prefetch counters stay at 0 even when loads succeed.
//!
//! **Batch path:** use worker indexing timing for bulk load cost; for HTTP-style prefetch breakdown,
//! run with `batch_prefetch_can_ingest_bodies` false so historic load queues per-message `Prefetch`
//! (not ideal for throughput), or extend `BatchPrefetch` with timing hooks.
//!
//! **Indexing timing** updates only when the search worker processes index intents. If bodies never
//! land (e.g. real-body `remote_id` mismatch), intents are never queued and indexing stats stay 0.
//!
//! For fixture-based testing (bypasses HTTP and decryption with pre-loaded bodies):
//!   cargo run -p mail-search-perf --example historic_load_test --features "foundation_search,foundation_search_lab_harness,foundation_search_index_timing" -- --fixture-path <path/to/fixture.jsonl> ...
//!
//! For real-bodies testing (chunked HTTPS bucket, keyed by Proton `remote_id`):
//!   cargo run -p mail-search-perf --example historic_load_test --features "foundation_search,foundation_search_lab_harness,foundation_search_index_timing" -- \
//!     --real-bodies-api --remote-fixture-config path/to/fixture_remote.config.json ...
//!   Or set env `FIXTURE_REMOTE_CONFIG_PATH` to that JSON (see `mail-common/fixture_remote.config.example.json`).
//!
//! Ephemeral index-only mode (no message metadata/body saves, no index intents, no SQLite writes):
//!   With fixture bodies:
//!   cargo run -p mail-search-perf --example historic_load_test --features "foundation_search,foundation_search_lab_harness,foundation_search_index_timing" -- \
//!     --real-bodies-api --remote-fixture-config path/to/fixture_remote.config.json --ephemeral-index-only ...
//!   Without fixtures (fetch + decrypt from API, zero SQLite message writes):
//!   cargo run -p mail-search-perf --example historic_load_test --features "foundation_search,foundation_search_lab_harness,foundation_search_index_timing" -- \
//!     --ephemeral-index-only ...
//!
//! The example will:
//! 1. Log in and create a MailUserContext
//! 2. Call historic_load_messages with the provided parameters
//! 3. Display the results (prefetch + fixture stats with lab harness; indexing timing section only if `foundation_search_index_timing` is in `--features`)
//! 4. Optionally persist the database for inspection (--persist-db)

#[path = "historic_load/core.rs"]
mod historic_load_core;
#[path = "historic_load/fixtures.rs"]
mod historic_load_fixtures;

use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use mail_historic_search_load::historic_load_messages;
use mail_stash::params;
use tempfile::TempDir;
use tracing::info;
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
    /// Maximum number of messages to process (default: 1000)
    #[clap(long, default_value = "1000")]
    max_messages: usize,
    /// Page size for fetching messages (default: 100)
    #[clap(long, default_value = "100")]
    page_size: usize,
    /// Persist the database to ./historic_load_test_db/ for inspection after run
    #[clap(long, default_value = "false")]
    persist_db: bool,
    /// Path to fixture JSONL file for fixture_bodies mode (bypasses HTTP and decryption)
    #[clap(long)]
    fixture_path: Option<String>,
    /// Use real-bodies loader (chunked HTTPS: `chunked_bodies` in remote fixture JSON)
    #[clap(long, default_value = "false")]
    real_bodies_api: bool,
    /// Path to UTF-8 JSON with `batch_api` / `chunked_bodies` (same shape as `fixture_remote.config.example.json`).
    /// Required for `--real-bodies-api` unless `FIXTURE_REMOTE_CONFIG_PATH` is set.
    #[clap(long)]
    remote_fixture_config: Option<PathBuf>,
    /// Number of concurrent batch requests for API-based body loading
    #[clap(long, default_value = "5")]
    api_concurrent_batches: usize,
    /// Disable telemetry event writes for cleaner perf runs
    #[clap(long, default_value = "false")]
    no_telemetry: bool,
    /// Perf-only path: skip all SQLite writes. Uses fixture bodies if available,
    /// otherwise fetches + decrypts from API directly. Indexes into Foundation Search only.
    #[clap(long, default_value = "false")]
    ephemeral_index_only: bool,
    /// Max concurrent API body fetches in ephemeral mode (default: 10)
    #[clap(long, default_value = "10")]
    ephemeral_concurrency: usize,
    /// Run a search query after the load and print results (e.g. --search-query "Youngsters")
    #[clap(long)]
    search_query: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Configure logging
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .parse_lossy(
            "historic_load_test=info,mail_common=warn,mail_historic_search_load::historic_load=info",
        );
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .init();

    let Args {
        username,
        password,
        email_password,
        max_messages,
        page_size,
        persist_db,
        fixture_path,
        real_bodies_api,
        remote_fixture_config,
        api_concurrent_batches,
        no_telemetry,
        ephemeral_index_only,
        ephemeral_concurrency,
        search_query,
    } = Args::parse();

    historic_load_fixtures::init_lab_search_fixtures(
        fixture_path.clone(),
        real_bodies_api,
        remote_fixture_config,
        api_concurrent_batches,
    );

    // Set up persistence directory if requested
    let persist_dir = if persist_db {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("historic_load_test_db");

        historic_load_core::remove_dir_all_if_exists(&dir)?;

        info!("Database will be persisted to: {:?}", dir);
        Some(dir)
    } else {
        None
    };

    // Create temporary directory for data
    let tmp_dir = TempDir::new().unwrap();
    info!("Using temporary directory: {:?}", tmp_dir.path());

    let ctx = historic_load_core::new_mail_context(&tmp_dir).await?;
    let user_ctx =
        historic_load_core::login_and_user_context(&ctx, username, password, email_password)
            .await?;

    if no_telemetry {
        let mut tether = user_ctx.user_stash().connection();
        tether
            .write_tx::<_, (), anyhow::Error>(async |tx| {
                tx.execute("UPDATE user_settings SET telemetry = 0", params![])
                    .await?;
                Ok(())
            })
            .await?;
        info!("Telemetry disabled for this run via user_settings.telemetry=0");
    }

    // Run historic load
    info!(
        "Starting historic load (max_messages: {}, page_size: {})...",
        max_messages, page_size
    );

    mail_search_perf::prefetch_timing::PrefetchStopwatch::reset_counters();
    mail_historic_search_load::ephemeral_timing::reset();

    // Reset indexing timing counters if foundation_search_index_timing is enabled.
    #[cfg(feature = "foundation_search_index_timing")]
    mail_search::indexing_timing::reset();

    let start_time = Instant::now();

    let result = if ephemeral_index_only {
        let e = mail_historic_search_load::ephemeral_index_only_messages(
            &user_ctx,
            None, // label_id: None = All Mail
            max_messages,
            page_size,
            ephemeral_concurrency,
            None, // continuation: start from newest
        )
        .await
        .map_err(|e| anyhow::anyhow!("Ephemeral historic load failed: {}", e))?;
        info!(
            "Ephemeral mode: skipped {} messages without fixture/real body",
            e.messages_skipped_missing_body
        );
        mail_historic_search_load::HistoricLoadResult {
            messages_fetched: e.messages_fetched,
            messages_indexed: e.messages_indexed,
            messages_prefetched: 0,
            oldest_saved_message_time: e.oldest_message_time,
            oldest_saved_message_remote_id: e.oldest_message_remote_id,
        }
    } else {
        historic_load_messages(
            &user_ctx,
            None, // label_id: None = All Mail
            Some(max_messages),
            Some(page_size),
            None, // continuation: start from newest
        )
        .await
        .map_err(|e| anyhow::anyhow!("Historic load failed: {}", e))?
    };

    let elapsed = start_time.elapsed();

    // Display results
    info!("Historic load completed!");
    info!("Time: {:.2}s", elapsed.as_secs_f64());
    info!("Messages fetched: {}", result.messages_fetched);
    info!("Messages indexed: {}", result.messages_indexed);
    info!("Messages prefetched: {}", result.messages_prefetched);
    if let Some(t) = result.oldest_saved_message_time {
        info!("Oldest message in fetched batch (unix secs): {}", t);
    }
    if let Some(id) = &result.oldest_saved_message_remote_id {
        info!("Oldest message remote id in fetched batch: {}", id);
    }

    if result.messages_fetched > 0 {
        info!(
            "Average time per message: {:.3}s",
            elapsed.as_secs_f64() / result.messages_fetched as f64
        );
    }

    let timing_stats = mail_search_perf::prefetch_timing::PrefetchTimingStats::snapshot();
    info!("\n{}", timing_stats);

    let total_measured = timing_stats.total_measured_time();
    if timing_stats.total_count > 0 {
        let prefetch_workers = user_ctx
            .get_service_opt::<mail_common::DefaultQueueExecutor>()
            .map(|q| q.prefetch_rollback_worker_count());
        match prefetch_workers {
            Some(n) => info!("Wall-clock analysis (prefetch rollback pool: {n} workers):"),
            None => info!("Wall-clock analysis (prefetch rollback pool: unknown):"),
        }
        info!(
            "  API fetch:             {:.1}% of measured time",
            (timing_stats.api_fetch.as_secs_f64() / total_measured.as_secs_f64()) * 100.0
        );
        info!(
            "  Metadata save/rebase:  {:.1}% of measured time",
            (timing_stats.metadata_save.as_secs_f64() / total_measured.as_secs_f64()) * 100.0
        );
        info!(
            "  Decrypt only:          {:.1}% of measured time",
            (timing_stats.decrypt_only.as_secs_f64() / total_measured.as_secs_f64()) * 100.0
        );
        info!(
            "  Body save + queue:     {:.1}% of measured time",
            (timing_stats.body_store_and_index_intent.as_secs_f64() / total_measured.as_secs_f64())
                * 100.0
        );
    }

    let real_stats = mail_search_perf::fixture_bodies::RealBodiesStats::snapshot();
    let fixture_stats = mail_search_perf::fixture_bodies::FixtureStats::snapshot();

    // Display detailed indexing timing breakdown if foundation_search_index_timing is enabled
    #[cfg(feature = "foundation_search_index_timing")]
    {
        let indexing_stats = mail_search::indexing_timing::IndexingTimingStats::snapshot();
        info!("\n{}", indexing_stats);

        if !ephemeral_index_only {
            info!("\nNon-Ephemeral Stage Timing Totals:");
            info!(
                "  Decrypt stage:                {:.2}s ({} operations)",
                timing_stats.decrypt_only.as_secs_f64(),
                timing_stats.total_count
            );
            info!(
                "  HTML strip stage:             {:.2}s ({} messages)",
                indexing_stats.html_strip_time.as_secs_f64(),
                indexing_stats.total_messages
            );
            info!(
                "  Foundation indexing only:     {:.2}s ({} messages)",
                indexing_stats.index_time.as_secs_f64(),
                indexing_stats.total_messages
            );
        }
    }

    if ephemeral_index_only {
        let mut ephemeral_timing =
            mail_historic_search_load::ephemeral_timing::EphemeralTimingStats::snapshot();
        if let Some(rs) = &real_stats {
            // Real-bodies mode decrypts lazily in fixture lookup path; merge it into ephemeral
            // decrypt totals so this section reports end-to-end stage costs.
            ephemeral_timing.decrypt_time += rs.decrypt_total;
            ephemeral_timing.decrypt_count += rs.decrypt_count as u64;
        }
        info!("\n{}", ephemeral_timing);
    }

    if let Some(real_stats) = real_stats {
        info!("\n{}", real_stats);
    } else if let Some(fixture_stats) = fixture_stats {
        info!("\n{}", fixture_stats);
        if fixture_stats.bodies_served > 0 {
            info!("  Average fixture body time: N/A (instant - bypasses HTTP/decrypt)");
        }
    }

    // Run a search query if requested
    if let Some(ref query) = search_query {
        info!("Running search query: {:?}", query);
        let search_start = Instant::now();
        match user_ctx
            .search_service()
            .search_local_with_metadata(query)
            .await
        {
            Ok(results) => {
                let search_elapsed = search_start.elapsed();
                info!(
                    "Search returned {} results in {:.1}ms",
                    results.len(),
                    search_elapsed.as_secs_f64() * 1000.0
                );
                for (i, entry) in results.iter().enumerate().take(20) {
                    info!(
                        "  [{}] id={} score={:.4}",
                        i + 1,
                        entry.identifier(),
                        entry.score(),
                    );
                }
                if results.len() > 20 {
                    info!("  ... and {} more", results.len() - 20);
                }
            }
            Err(e) => {
                info!("Search failed: {e}");
            }
        }
    }

    // Persist database if requested
    if let Some(ref persist_dir) = persist_dir {
        historic_load_core::persist_mail_databases(&tmp_dir, persist_dir, user_ctx.user_id())
            .await?;
        info!("Database persisted to: {:?}", persist_dir);
        info!("Inspect with: ./mail/mail-search-perf/scripts/inspect-historic-load-db.sh");
    }

    Ok(())
}
