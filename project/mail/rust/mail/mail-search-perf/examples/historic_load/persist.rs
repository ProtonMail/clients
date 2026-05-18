//! DB persist helpers for the `historic_load_test` example only.

use std::path::Path;
use std::process::Command;

use anyhow::Context;
use mail_core_api::services::proton::UserId;
use tempfile::TempDir;
use tracing::{info, warn};

/// Remove a directory if it exists (e.g. fresh persist dir).
pub fn remove_dir_all_if_exists(dir: &Path) -> anyhow::Result<()> {
    if dir.exists() {
        info!("Cleaning existing database directory: {dir:?}");
        std::fs::remove_dir_all(dir).with_context(|| format!("remove_dir_all {dir:?}"))?;
    }
    Ok(())
}

/// WAL-checkpoint the user DB (if present), then copy session and user SQLite files into `persist_dir`.
pub async fn persist_mail_databases(
    tmp_dir: &TempDir,
    persist_dir: &Path,
    user_id: &UserId,
) -> anyhow::Result<()> {
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let session_db_path = tmp_dir.path().join("session").join("account.db");
    let user_db_path = tmp_dir.path().join("user").join(format!("{user_id}.db"));

    if user_db_path.exists() {
        let output = Command::new("sqlite3")
            .arg(&user_db_path)
            .arg("PRAGMA wal_checkpoint(FULL);")
            .output();

        match output {
            Ok(out) if out.status.success() => {
                info!("WAL checkpointed successfully");
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!("WAL checkpoint warning: {}", stderr);
            }
            Err(e) => {
                warn!(
                    "Failed to run sqlite3 checkpoint: {} (is sqlite3 installed?)",
                    e
                );
            }
        }
    }

    let session_dir = persist_dir.join("session");
    let user_dir = persist_dir.join("user");
    std::fs::create_dir_all(&session_dir)
        .with_context(|| format!("create_dir_all {session_dir:?}"))?;
    std::fs::create_dir_all(&user_dir).with_context(|| format!("create_dir_all {user_dir:?}"))?;

    if session_db_path.exists() {
        let dest = session_dir.join("account.db");
        std::fs::copy(&session_db_path, &dest)
            .with_context(|| format!("copy session DB {:?} -> {:?}", session_db_path, dest))?;
        info!("Copied session DB to {dest:?}");
    } else {
        warn!("Session DB not found at: {session_db_path:?}");
    }

    if user_db_path.exists() {
        let dest = user_dir.join(format!("{user_id}.db"));
        std::fs::copy(&user_db_path, &dest)
            .with_context(|| format!("copy user DB {:?} -> {:?}", user_db_path, dest))?;
        info!("Copied user DB to {dest:?}");
    } else {
        warn!("User DB not found at: {user_db_path:?}");
    }

    Ok(())
}

/// Copy persisted session/user DBs into a fresh temp dir so a later run can resume checkpoints/index.
///
/// Returns `true` if anything was restored.
pub fn restore_mail_databases_if_present(
    persist_dir: &Path,
    tmp_dir: &TempDir,
) -> anyhow::Result<bool> {
    let session_src = persist_dir.join("session").join("account.db");
    let user_src_dir = persist_dir.join("user");
    let mut restored = false;

    if session_src.exists() {
        let session_dst_dir = tmp_dir.path().join("session");
        std::fs::create_dir_all(&session_dst_dir)
            .with_context(|| format!("create_dir_all {session_dst_dir:?}"))?;
        let dest = session_dst_dir.join("account.db");
        std::fs::copy(&session_src, &dest)
            .with_context(|| format!("copy session DB {:?} -> {:?}", session_src, dest))?;
        info!("Restored session DB from {session_src:?}");
        restored = true;
    }

    if user_src_dir.is_dir() {
        let user_dst_dir = tmp_dir.path().join("user");
        std::fs::create_dir_all(&user_dst_dir)
            .with_context(|| format!("create_dir_all {user_dst_dir:?}"))?;
        for entry in std::fs::read_dir(&user_src_dir)
            .with_context(|| format!("read_dir {user_src_dir:?}"))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "db") {
                let dest = user_dst_dir.join(entry.file_name());
                std::fs::copy(&path, &dest)
                    .with_context(|| format!("copy user DB {:?} -> {:?}", path, dest))?;
                info!("Restored user DB from {path:?}");
                restored = true;
            }
        }
    }

    Ok(restored)
}
