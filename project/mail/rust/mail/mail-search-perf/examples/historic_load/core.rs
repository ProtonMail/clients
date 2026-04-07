//! Shared helpers for `historic_load_test` and `historic_load_trial` examples.
//!

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use anyhow::Context;
use mail_common::{MailContext, MailUserContext};
use mail_core_api::services::proton::UserId;
use mail_core_common::Origin;
use mail_core_common::datatypes::{ApiConfig, AppDetails};
use mail_core_common::db::account::SessionEncryptionKey;
use mail_core_common::event_loop::EventPollMode;
use mail_core_common::os::{InMemoryKeyChain, KeyChainExt};
use mail_issue_reporter_service::NoopIssueReporter;
use mail_log_service::LogService;
use tempfile::TempDir;
use tokio::runtime;
use tracing::{info, warn};

pub async fn new_mail_context(tmp_dir: &TempDir) -> anyhow::Result<Arc<MailContext>> {
    let keychain = InMemoryKeyChain::default();
    let key = SessionEncryptionKey::random();
    keychain
        .store(key)
        .map_err(|e| anyhow::anyhow!("keychain store: {}", e))?;

    let config = mail_log_service::Config::builder()
        .name("log".into())
        .directory(tmp_dir.path().into())
        .build();
    let api_config = ApiConfig {
        app_details: AppDetails {
            platform: "ios".into(),
            product: "mail".into(),
            version: "7.1.0".into(),
        },
        ..Default::default()
    };

    MailContext::new(
        Origin::App,
        runtime::Handle::current(),
        tmp_dir.path().join("session"),
        tmp_dir.path().join("user"),
        tmp_dir.path().join("core_cache"),
        tmp_dir.path().join("mail_cache"),
        50 * 1024 * 1024,
        Arc::new(keychain),
        api_config,
        None,
        None,
        LogService::new(config),
        EventPollMode::Manual,
        Default::default(),
        Arc::new(NoopIssueReporter),
    )
    .await
    .map_err(|e| anyhow::anyhow!("MailContext::new: {}", e))
}

pub async fn login_and_user_context(
    ctx: &Arc<MailContext>,
    username: String,
    password: String,
    email_password: Option<String>,
) -> anyhow::Result<Arc<MailUserContext>> {
    info!("Logging in as {username}...");
    let mut flow = ctx.new_login_flow().await.context("new_login_flow")?;

    flow.login_with_credentials(username, password, None)
        .await
        .context("login_with_credentials")?;

    if flow.is_awaiting_mailbox_password() {
        if let Some(ref mbp) = email_password {
            flow.submit_mailbox_password(mbp.clone())
                .await
                .context("submit_mailbox_password (initial)")?;
        } else {
            anyhow::bail!("Account requires mailbox password. Please provide --email-password");
        }
    }

    while !flow.is_logged_in() {
        if flow.is_awaiting_2fa() {
            anyhow::bail!("Account requires 2FA. This example doesn't support 2FA.");
        } else if flow.is_awaiting_new_password() {
            anyhow::bail!(
                "Account requires new password. This example doesn't support password reset."
            );
        } else if flow.is_awaiting_mailbox_password() {
            if let Some(ref mbp) = email_password {
                flow.submit_mailbox_password(mbp.clone())
                    .await
                    .context("submit_mailbox_password (wait loop)")?;
            } else {
                anyhow::bail!("Account requires mailbox password. Please provide --email-password");
            }
        } else {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    let user_ctx = ctx
        .user_context_from_login_flow(&mut flow)
        .await
        .context("user_context_from_login_flow")?;
    info!("Logged in successfully");
    Ok(user_ctx)
}

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
