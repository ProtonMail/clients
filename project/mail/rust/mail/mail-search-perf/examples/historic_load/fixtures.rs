//! Lab fixture / real-bodies init for `historic_load_test` (enabled whenever `foundation_search` is on this crate).

use std::path::{Path, PathBuf};

use tracing::{info, warn};

fn remote_fixture_config_path(cli: Option<&PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(p) = cli {
        return Ok(p.clone());
    }
    std::env::var("FIXTURE_REMOTE_CONFIG_PATH")
        .map(PathBuf::from)
        .map_err(|_| {
            anyhow::anyhow!(
                "Set --remote-fixture-config <path> or FIXTURE_REMOTE_CONFIG_PATH (JSON with chunked_bodies for --real-bodies-api)"
            )
        })
}

/// Initialize JSONL / real-bodies fixtures for perf runs (`foundation_search` on this crate).
pub fn init_lab_search_fixtures(
    fixture_path: Option<String>,
    real_bodies_api: bool,
    remote_fixture_config: Option<PathBuf>,
    api_concurrent_batches: usize,
) {
    let _ = api_concurrent_batches; // reserved for future batch-api fixture mode
    if real_bodies_api {
        let cfg_path = remote_fixture_config_path(remote_fixture_config.as_ref())
            .unwrap_or_else(|e| panic!("{e}"));
        info!(
            "Initializing real bodies from chunked HTTP (config: {})...",
            cfg_path.display()
        );
        let file = mail_search_perf::fixture_bodies::load_remote_fixture_config_from_path(
            Path::new(&cfg_path),
        )
        .unwrap_or_else(|e| panic!("Failed to load {}: {}", cfg_path.display(), e));
        let chunked = file.chunked_bodies.unwrap_or_else(|| {
            panic!(
                "Fixture config {} has no `chunked_bodies` section",
                cfg_path.display()
            )
        });
        mail_search_perf::fixture_bodies::initialize_real_bodies_api(chunked).unwrap_or_else(|e| {
            panic!("Failed to initialize real bodies: {}", e);
        });
        info!("Real bodies loader started (chunks will be downloaded in background)");
    } else if let Some(ref path) = fixture_path {
        info!("Loading fixture bodies from: {path}");
        mail_search_perf::fixture_bodies::initialize(Some(Path::new(path))).unwrap_or_else(|e| {
            panic!("Failed to initialize fixture bodies from {path}: {}", e);
        });
        info!(
            "Loaded {} fixture bodies",
            mail_search_perf::fixture_bodies::total_fixtures()
        );
    } else {
        warn!(
            "Neither --real-bodies-api nor --fixture-path provided; message bodies will use normal HTTP fetch."
        );
    }
}
