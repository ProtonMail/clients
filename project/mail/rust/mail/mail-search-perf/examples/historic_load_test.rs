//! Historic load perf / smoke harness driving the **production**
//! `ContentSearchIndexingOrchestrator` (the multi-batch loop now owned by
//! Rust, not the harness).
//!
//! This example used to call the single-batch
//! `ephemeral_index_only_messages` helper directly and let the operator
//! orchestrate batches by re-running the binary. The orchestrator now owns
//! pagination, resume, and mailbox-end detection; this harness only sets
//! the user's enable preference, configures the orchestrator (concurrency
//! knob, optional perf ceilings), subscribes to the live-query watch for
//! progress reporting, and waits for the run to finish.
//!
//! ```text
//! # Fresh run, no persistence (whole mailbox up to mailbox-end)
//! cargo run -p mail-search-perf --example historic_load_test \
//!   --features foundation_search -- --username <email> --password <pass>
//!
//! # Fresh run, persist DB to ./historic_load_test_db/
//! cargo run -p mail-search-perf --example historic_load_test \
//!   --features foundation_search -- --username <email> --password <pass> --persist-db
//!
//! # Resume from the persisted DB (orchestrator picks up at the checkpoint)
//! cargo run -p mail-search-perf --example historic_load_test \
//!   --features foundation_search -- --username <email> --password <pass> \
//!   --persist-db --reuse-db
//!
//! # Perf: cap at 1000 fresh messages, then stop
//! cargo run -p mail-search-perf --example historic_load_test \
//!   --features foundation_search -- --username <email> --password <pass> --ceiling 1000
//!
//! # Perf: stop after 30 seconds of wall-clock work
//! cargo run -p mail-search-perf --example historic_load_test \
//!   --features foundation_search -- --username <email> --password <pass> --cancel-after 30
//!
//! # Inspect DB: ./mail/mail-search-perf/scripts/inspect-historic-load-db.sh
//! ```
//!
//! Add `foundation_search_index_timing` for indexing timing output.

#[path = "historic_load/core.rs"]
mod historic_load_core;
#[path = "historic_load/persist.rs"]
mod historic_load_persist;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use mail_common::search::{ContentSearchIndexingProgress, ContentSearchIndexingStatus};
use mail_historic_ephemeral_load::{
    ContentSearchIndexingOrchestrator, ContentSearchIndexingOrchestratorConfig,
    ContentSearchStartOutcome, EPHEMERAL_HISTORIC_LOAD_BATCH_SIZE,
};
use mail_stash::params;
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
    /// Optional mailbox password (required if account uses separate mailbox password).
    #[clap(long)]
    email_password: Option<String>,
    /// Persist the database to ./historic_load_test_db/ for inspection after run.
    #[clap(long, default_value = "false")]
    persist_db: bool,
    /// With `--persist-db`, keep `historic_load_test_db/` and restore it before login;
    /// the orchestrator resumes from the persisted checkpoint automatically.
    #[clap(long, default_value = "false")]
    reuse_db: bool,
    /// Disable telemetry event writes for cleaner perf runs.
    #[clap(long, default_value = "false")]
    no_telemetry: bool,
    /// Max concurrent API body fetches passed through to the orchestrator's batch runner.
    #[clap(long, default_value = "10")]
    ephemeral_concurrency: usize,
    /// Perf cap: cancel the orchestrator once **delta** indexed messages
    /// for this run reaches this value. Counted against the snapshot taken
    /// before `start_indexing`; cumulative counters from previous runs
    /// (when `--reuse-db`) are excluded.
    #[clap(long)]
    ceiling: Option<u64>,
    /// Perf cap: cancel the orchestrator after this many wall-clock seconds.
    #[clap(long)]
    cancel_after: Option<u64>,
    /// Run a search query after the load and print results (e.g. `--search-query "Youngsters"`).
    #[clap(long)]
    search_query: Option<String>,
}

/// Block until the first OS shutdown signal arrives.
///
/// On Unix this races Ctrl-C against SIGTERM (so `kill <pid>` and systemd
/// stop also trigger graceful cancel). On non-Unix platforms only Ctrl-C is
/// portable.
///
/// Failure to install the SIGTERM handler is non-fatal — we fall back to
/// Ctrl-C only and log a warning. The harness must never be unable to
/// start indexing just because signal-handler registration failed.
async fn wait_for_shutdown_signal() -> &'static str {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => tokio::select! {
                _ = tokio::signal::ctrl_c() => "Ctrl-C",
                _ = sigterm.recv() => "SIGTERM",
            },
            Err(e) => {
                warn!("failed to install SIGTERM handler ({e}); falling back to Ctrl-C only");
                let _ = tokio::signal::ctrl_c().await;
                "Ctrl-C"
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        "Ctrl-C"
    }
}

fn format_estimated_percent(p: &ContentSearchIndexingProgress) -> String {
    match p.estimated_fraction {
        Some(fraction) => format!("{}%", (fraction * 100.0).round() as u64),
        None => "n/a".to_owned(),
    }
}

fn log_progress_snapshot(prefix: &str, baseline_indexed: u64, p: &ContentSearchIndexingProgress) {
    let delta_indexed = p.messages_indexed_total.saturating_sub(baseline_indexed);
    info!(
        "{prefix}: status={:?} estimated={} batches_completed={} indexed +{} (cumulative indexed/fetched/skipped = {}/{}/{})",
        p.status,
        format_estimated_percent(p),
        p.batches_completed,
        delta_indexed,
        p.messages_indexed_total,
        p.messages_fetched_total,
        p.messages_skipped_total,
    );
    if let Some(err) = &p.last_error {
        info!("  last_error: {err}");
    }
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
        persist_db,
        reuse_db,
        no_telemetry,
        ephemeral_concurrency,
        ceiling,
        cancel_after,
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
        None
    };

    let tmp_dir = TempDir::new().unwrap();
    info!("Using temporary directory: {:?}", tmp_dir.path());

    if let Some(ref dir) = persist_dir
        && reuse_db
    {
        historic_load_persist::restore_mail_databases_if_present(dir, &tmp_dir)?;
    }

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

    // Honour the spec's explicit-enable contract: persist the preference
    // before any start. This is a no-op if the user already enabled it.
    user_ctx.search_service().set_indexing_enabled(true).await?;

    // Snapshot the durable counters before we start. With `--reuse-db` the
    // row already holds cumulative totals from previous runs; subtracting
    // this baseline lets `--ceiling` and the per-run report measure only
    // the work done in this invocation.
    let baseline = user_ctx.search_service().load_indexing_progress().await?;
    let baseline_indexed = baseline.messages_indexed_total;
    info!(
        "Initial state: status={:?} estimated={} batches_completed={} cumulative indexed/fetched/skipped = {}/{}/{}",
        baseline.status,
        format_estimated_percent(&baseline),
        baseline.batches_completed,
        baseline.messages_indexed_total,
        baseline.messages_fetched_total,
        baseline.messages_skipped_total,
    );

    // Build a local orchestrator with the harness-chosen concurrency rather
    // than the one wired into the `MailUserContext` services map: the perf
    // run wants a knob (`--ephemeral-concurrency`) that the production
    // default provider does not expose, and constructing directly keeps the
    // perf-tuned config out of the per-session service registry.
    let orchestrator_config = ContentSearchIndexingOrchestratorConfig {
        concurrent_body_fetches: ephemeral_concurrency,
        ..Default::default()
    };
    let orchestrator = Arc::new(ContentSearchIndexingOrchestrator::with_config(
        orchestrator_config,
    ));

    // Live-query watch for progress reporting + ceiling enforcement.
    let watch_handle = user_ctx
        .search_service()
        .watch_indexing_state()
        .await
        .map_err(|e| anyhow::anyhow!("watch_indexing_state: {e}"))?;
    let progress_search_service = user_ctx.search_service().clone();
    let progress_orchestrator = orchestrator.clone();
    let progress_task = tokio::spawn(async move {
        let receiver = watch_handle.receiver();
        while receiver.recv_async().await.is_ok() {
            match progress_search_service.load_indexing_progress().await {
                Ok(progress) => {
                    log_progress_snapshot("watch", baseline_indexed, &progress);

                    if let Some(c) = ceiling {
                        let delta = progress
                            .messages_indexed_total
                            .saturating_sub(baseline_indexed);
                        if delta >= c {
                            info!(
                                "ceiling reached (delta indexed {delta} >= ceiling {c}); cancelling orchestrator"
                            );
                            progress_orchestrator.cancel();
                        }
                    }
                }
                Err(e) => warn!("progress load error: {e}"),
            }
        }
        // Channel disconnected (orchestrator + stash being torn down).
    });

    // Cancel-after timer.
    let cancel_after_task = cancel_after.map(|secs| {
        let orchestrator = orchestrator.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(secs)).await;
            info!("--cancel-after {secs}s reached; cancelling orchestrator");
            orchestrator.cancel();
        })
    });

    // Shutdown-signal watcher (Ctrl-C on every platform, plus SIGTERM on
    // Unix). Calls `cancel()` so the orchestrator's loop returns
    // `IndexingRunOutcome::Cancelled` at its next cooperative yield point
    // — including mid-batch cooperative cancel — and the durable status row
    // settles on `Interrupted` rather than `Ongoing`.
    let signal_orchestrator = orchestrator.clone();
    let signal_task = tokio::spawn(async move {
        let received = wait_for_shutdown_signal().await;
        info!(
            "{received} received; cancelling orchestrator (graceful, will yield at next batch boundary or sooner)"
        );
        signal_orchestrator.cancel();
    });

    info!(
        "Starting orchestrator (batch_size={}, concurrent_body_fetches={}, ceiling={:?}, cancel_after={:?}s)…",
        EPHEMERAL_HISTORIC_LOAD_BATCH_SIZE, ephemeral_concurrency, ceiling, cancel_after,
    );

    mail_search_perf::prefetch_timing::PrefetchStopwatch::reset_counters();

    #[cfg(feature = "foundation_search_index_timing")]
    mail_search::indexing_timing::reset();

    let start_time = Instant::now();
    let outcome = orchestrator
        .start(user_ctx.clone())
        .await
        .map_err(|e| anyhow::anyhow!("orchestrator.start: {e}"))?;
    info!("start_indexing outcome: {outcome:?}");

    if matches!(outcome, ContentSearchStartOutcome::Started) {
        // Wait for the spawned task to release the in-process slot.
        // `is_running()` flips to `false` once the orchestrator's
        // Reservation drops (natural completion, cancel, fatal exit,
        // or partial-batch interrupt).
        while orchestrator.is_running() {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    let elapsed = start_time.elapsed();

    // Tear down the live-query watch, the pending cancel-after timer, and
    // the shutdown-signal watcher before reading the final state to keep
    // tracing output deterministic.
    progress_task.abort();
    if let Some(handle) = cancel_after_task {
        handle.abort();
    }
    signal_task.abort();

    let final_progress = user_ctx.search_service().load_indexing_progress().await?;
    let delta_indexed = final_progress
        .messages_indexed_total
        .saturating_sub(baseline_indexed);
    let delta_fetched = final_progress
        .messages_fetched_total
        .saturating_sub(baseline.messages_fetched_total);
    let delta_skipped = final_progress
        .messages_skipped_total
        .saturating_sub(baseline.messages_skipped_total);
    let delta_batches = final_progress
        .batches_completed
        .saturating_sub(baseline.batches_completed);

    info!("Historic load finished.");
    info!("Wall-clock: {:.2}s", elapsed.as_secs_f64());
    info!("Final status: {:?}", final_progress.status);
    info!(
        "Estimated progress: {}",
        format_estimated_percent(&final_progress)
    );
    if let Some(err) = &final_progress.last_error {
        info!("last_error: {err}");
    }
    info!(
        "This run: batches +{delta_batches}, indexed +{delta_indexed}, fetched +{delta_fetched}, skipped +{delta_skipped}"
    );
    info!(
        "Cumulative: batches={} indexed/fetched/skipped = {}/{}/{}",
        final_progress.batches_completed,
        final_progress.messages_indexed_total,
        final_progress.messages_fetched_total,
        final_progress.messages_skipped_total,
    );

    if delta_fetched > 0 {
        info!(
            "Average time per fetched message: {:.3}s",
            elapsed.as_secs_f64() / delta_fetched as f64,
        );
    }

    if final_progress.status == ContentSearchIndexingStatus::Completed {
        info!("Mailbox fully indexed (status = Completed).");
    }

    let timing_stats = mail_search_perf::prefetch_timing::PrefetchTimingStats::snapshot();
    info!("\n{}", timing_stats);

    let total_measured = timing_stats.total_measured_time();
    if timing_stats.total_count > 0 {
        info!("Wall-clock analysis (measured phases vs total measured time):");
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
        info!(
            "Inspect indexing state: sqlite3 {:?}/user/*.db \"SELECT * FROM content_search_indexing_state;\"",
            persist_dir
        );
        info!("Inspect with: ./mail/mail-search-perf/scripts/inspect-historic-load-db.sh");
    }

    Ok(())
}
