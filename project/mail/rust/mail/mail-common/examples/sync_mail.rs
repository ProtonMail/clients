use std::sync::Arc;

use clap::Parser;
use mail_common::{
    BackwardSync, BackwardSyncParams, MailContext, SyncEvent, SyncOutcome, SyncService,
};
use mail_core_common::Origin;
use mail_core_common::datatypes::{ApiConfig, AppDetails};
use mail_core_common::db::account::SessionEncryptionKey;
use mail_core_common::event_loop::EventPollMode;
use mail_core_common::os::{InMemoryKeyChain, KeyChainExt};
use mail_issue_reporter_service::NoopIssueReporter;
use mail_log_service::LogService;
use tempfile::TempDir;
use tokio::runtime;
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
}
#[tokio::main]
async fn main() {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .parse_lossy(
            "info,mail_sqlite3=debug,\
                mail_core_common=debug,mail_common=debug,\
                core_event_loop=debug,mail_core_api=debug,\
                mail_action_queue=debug,mail_api=debug,\
                mail_stash=error",
        );
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .init();

    let Args { username, password } = Args::parse();
    let tmp_dir = TempDir::new().unwrap();

    let keychain = InMemoryKeyChain::default();
    let key = SessionEncryptionKey::random();
    keychain.store(key).unwrap();

    info!("TMP DIR: {:?}", tmp_dir.path());

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

    let ctx = MailContext::new(
        Origin::App,
        runtime::Handle::current(),
        tmp_dir.path().join("session"),
        tmp_dir.path().join("user"),
        tmp_dir.path().join("core_cache"),
        tmp_dir.path().join("mail_cache"),
        50 * 1204 * 1024,
        None,
        Arc::new(keychain),
        api_config,
        None,
        None,
        LogService::new(config),
        EventPollMode::Manual,
        Default::default(),
        Arc::new(NoopIssueReporter),
        None,
    )
    .await
    .unwrap();

    let mut flow = ctx.new_login_flow().await.unwrap();

    flow.login_with_credentials(username, password, None)
        .await
        .unwrap();

    let user_ctx = ctx.user_context_from_login_flow(&mut flow).await.unwrap();

    let sync_params = BackwardSyncParams {
        page_size: std::num::NonZeroUsize::new(100).unwrap(),
        chunk_split: None,
    };

    let instant = std::time::Instant::now();

    let service = SyncService::new(
        user_ctx.core_context().task_service().task_service(),
        user_ctx.user_event_service(),
        BackwardSync::new(sync_params),
        Arc::downgrade(&user_ctx),
    );

    let mut subscriber = user_ctx
        .user_event_service()
        .subscribe::<SyncEvent>()
        .unwrap();

    service.start().await.unwrap();

    while let Ok(event) = subscriber.next().await {
        match event {
            SyncEvent::Started => tracing::info!("Sync started"),
            SyncEvent::Stopped => {
                tracing::info!("Sync stopped");
                break;
            }
            SyncEvent::Completed(SyncOutcome::Success) => break,
            SyncEvent::Completed(SyncOutcome::RetryableFailure(e)) => {
                tracing::error!("Sync failed (rety): {e}")
            }
            SyncEvent::Completed(SyncOutcome::CriticalFailure(e)) => {
                tracing::error!("Sync failed: {e}")
            }
        }
    }

    tracing::info!("Sync took: {:?}", instant.elapsed());
}
