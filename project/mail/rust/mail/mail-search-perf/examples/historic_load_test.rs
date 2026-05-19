//! Historic load smoke / perf example (ephemeral path: metadata rows + Foundation Search, no bodies).
//!
//! Offline JSONL / remote fixture body substitution was removed; this mode uses the live API.
//!
//! ```text
//! # First batch (saves checkpoint + index when --persist-db)
//! cargo run -p mail-search-perf --example historic_load_test --features foundation_search -- \\
//!   --username <email> --password <pass> \\
//!   --max-messages 100 --persist-db
//!
//! # Next older batch (reuse DB + implicit checkpoint)
//! cargo run -p mail-search-perf --example historic_load_test --features foundation_search -- \\
//!   --username <email> --password <pass> \\
//!   --max-messages 100 --persist-db --reuse-db --resume-from-checkpoint
//! ```
//!
//! Add `foundation_search_index_timing` for indexing timing output.

#[path = "historic_load/core.rs"]
mod historic_load_core;
#[path = "historic_load/persist.rs"]
mod historic_load_persist;

use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use mail_historic_ephemeral_load::{EphemeralHistoricLoadResult, ephemeral_index_only_messages};
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
    /// With `--persist-db`, keep `historic_load_test_db/` and restore it before login (for resume runs)
    #[clap(long, default_value = "false")]
    reuse_db: bool,
    /// Load the saved SQLite checkpoint and fetch the next older messages (All Mail label)
    #[clap(long, default_value = "false")]
    resume_from_checkpoint: bool,
    /// Disable telemetry event writes for cleaner perf runs
    #[clap(long, default_value = "false")]
    no_telemetry: bool,
    /// Max concurrent API body fetches (default: 10)
    #[clap(long, default_value = "10")]
    ephemeral_concurrency: usize,
    /// Run a search query after the load and print results (e.g. --search-query "Youngsters")
    #[clap(long)]
    search_query: Option<String>,
}

async fn log_stored_checkpoint(
    user_ctx: &mail_common::MailUserContext,
    heading: &str,
) -> anyhow::Result<()> {
    match user_ctx
        .search_service()
        .load_ephemeral_historic_checkpoint()
        .await
    {
        Ok(Some(cp)) => {
            info!(
                "{heading}: anchor_time={} anchor_message_id={}",
                cp.anchor_time, cp.anchor_message_id
            );
        }
        Ok(None) => info!("{heading}: (no checkpoint row)"),
        Err(e) => info!("{heading}: failed to read ({e})"),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .parse_lossy("historic_load_test=info,mail_common=warn");
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
        reuse_db,
        resume_from_checkpoint,
        no_telemetry,
        ephemeral_concurrency,
        search_query,
    } = Args::parse();

    if reuse_db && !persist_db {
        anyhow::bail!("--reuse-db requires --persist-db");
    }

    let persist_dir = if persist_db {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("historic_load_test_db");

        if reuse_db {
            if dir.exists() {
                info!("Reusing persisted database directory: {dir:?}");
            } else {
                info!("--reuse-db set but {dir:?} does not exist yet; starting fresh");
            }
        } else {
            historic_load_persist::remove_dir_all_if_exists(&dir)?;
            info!("Database will be persisted to: {dir:?}");
        }
        Some(dir)
    } else {
        if reuse_db || resume_from_checkpoint {
            anyhow::bail!("--reuse-db and --resume-from-checkpoint require --persist-db");
        }
        None
    };

    let tmp_dir = TempDir::new().unwrap();
    info!("Using temporary directory: {:?}", tmp_dir.path());

    if let Some(ref dir) = persist_dir
        && reuse_db
    {
        let restored = historic_load_persist::restore_mail_databases_if_present(dir, &tmp_dir)?;
        if resume_from_checkpoint && !restored {
            anyhow::bail!(
                "--resume-from-checkpoint with --reuse-db but no database found in {dir:?}"
            );
        }
    }

    let ctx = historic_load_core::new_mail_context(&tmp_dir).await?;
    let user_ctx =
        historic_load_core::login_and_user_context(&ctx, username, password, email_password)
            .await?;

    if resume_from_checkpoint {
        log_stored_checkpoint(&user_ctx, "Checkpoint before load").await?;
    }

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

    info!(
        "Starting ephemeral historic load (max_messages: {}, page_size: {}, resume_from_checkpoint: {})...",
        max_messages, page_size, resume_from_checkpoint
    );

    mail_search_perf::prefetch_timing::PrefetchStopwatch::reset_counters();

    #[cfg(feature = "foundation_search_index_timing")]
    mail_search::indexing_timing::reset();

    let start_time = Instant::now();

    let result: EphemeralHistoricLoadResult = ephemeral_index_only_messages(
        &user_ctx,
        max_messages,
        page_size,
        ephemeral_concurrency,
        None,
        resume_from_checkpoint,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Ephemeral historic load failed: {}", e))?;

    info!(
        "Skipped {} messages (missing body / errors)",
        result.messages_skipped_missing_body
    );

    let elapsed = start_time.elapsed();

    info!("Historic load completed!");
    info!("Time: {:.2}s", elapsed.as_secs_f64());
    info!("Messages fetched: {}", result.messages_fetched);
    info!(
        "Messages metadata saved: {}",
        result.messages_metadata_saved
    );
    info!("Messages indexed: {}", result.messages_indexed);
    if let Some(t) = result.oldest_message_time {
        info!("Oldest message in fetched batch (unix secs): {}", t);
    }
    if let Some(id) = &result.oldest_message_remote_id {
        info!("Oldest message remote id in fetched batch: {}", id);
    }

    log_stored_checkpoint(&user_ctx, "Checkpoint after load").await?;

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
        info!("Wall-clock analysis (measured phases vs total wall time):");
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

    #[cfg(feature = "foundation_search_index_timing")]
    {
        let indexing_stats = mail_search::indexing_timing::IndexingTimingStats::snapshot();
        info!("\n{}", indexing_stats);
    }

    info!("\n{}", result.timing);

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

    if let Some(ref persist_dir) = persist_dir {
        historic_load_persist::persist_mail_databases(&tmp_dir, persist_dir, user_ctx.user_id())
            .await?;
        info!("Database persisted to: {:?}", persist_dir);
        info!(
            "Inspect checkpoint: sqlite3 {:?}/user/*.db \"SELECT * FROM ephemeral_historic_load_checkpoint;\"",
            persist_dir
        );
        info!("Inspect with: ./mail/mail-search-perf/scripts/inspect-historic-load-db.sh");
    }

    Ok(())
}
