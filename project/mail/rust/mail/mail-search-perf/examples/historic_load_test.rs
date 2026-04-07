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

    // Run historic load
    info!(
        "Starting historic load (max_messages: {}, page_size: {})...",
        max_messages, page_size
    );

    mail_search_perf::prefetch_timing::PrefetchStopwatch::reset_counters();

    // Reset indexing timing counters if foundation_search_index_timing is enabled.
    #[cfg(feature = "foundation_search_index_timing")]
    mail_search::indexing_timing::reset();

    let start_time = Instant::now();

    let result = historic_load_messages(
        &user_ctx,
        None, // label_id: None = All Mail
        Some(max_messages),
        Some(page_size),
    )
    .await
    .map_err(|e| anyhow::anyhow!("Historic load failed: {}", e))?;

    let elapsed = start_time.elapsed();

    // Display results
    info!("Historic load completed!");
    info!("Time: {:.2}s", elapsed.as_secs_f64());
    info!("Messages fetched: {}", result.messages_fetched);
    info!("Messages indexed: {}", result.messages_indexed);
    info!("Messages prefetched: {}", result.messages_prefetched);

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
        info!("Wall-clock analysis (with {} workers):", 6); // Assuming pool_size=6
        info!(
            "  HTTP + metadata save: {:.1}% of measured time",
            (timing_stats.http_and_metadata_save.as_secs_f64() / total_measured.as_secs_f64())
                * 100.0
        );
        info!(
            "  Decrypt + body save:  {:.1}% of measured time",
            (timing_stats.decrypt_and_body_save.as_secs_f64() / total_measured.as_secs_f64())
                * 100.0
        );
    }

    // Display detailed indexing timing breakdown if foundation_search_index_timing is enabled
    #[cfg(feature = "foundation_search_index_timing")]
    {
        let indexing_stats = mail_search::indexing_timing::IndexingTimingStats::snapshot();
        info!("\n{}", indexing_stats);
    }

    if let Some(real_stats) = mail_search_perf::fixture_bodies::RealBodiesStats::snapshot() {
        info!("\n{}", real_stats);
    } else if let Some(fixture_stats) = mail_search_perf::fixture_bodies::FixtureStats::snapshot() {
        info!("\n{}", fixture_stats);
        if fixture_stats.bodies_served > 0 {
            info!("  Average fixture body time: N/A (instant - bypasses HTTP/decrypt)");
        }
    }

    // Persist database if requested
    if let Some(ref persist_dir) = persist_dir {
        historic_load_core::persist_mail_databases(&tmp_dir, persist_dir, user_ctx.user_id())
            .await?;
        info!("Database persisted to: {:?}", persist_dir);
        info!("Inspect with: ./scripts/inspect-historic-load-db.sh");
    }

    Ok(())
}
