use crate::db::offline_migrations::run as migrate_mail_db;
use mail_core_common::db::migrations::migrate_core_db;
use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashConfiguration};
use tempfile::{TempDir, tempdir};
use tracing::subscriber::set_global_default;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, registry};

pub async fn new_test_connection() -> Stash<UserDb> {
    _ = set_global_default(
        registry()
            .with(EnvFilter::new("debug"))
            .with(layer().with_test_writer()),
    );

    let mail_stash = Stash::new(StashConfiguration::test()).unwrap();

    migrate_core_db(&mail_stash).await.unwrap();
    migrate_mail_db(&mail_stash).await.unwrap();

    mail_stash
}

pub async fn new_test_connection_file() -> (Stash<UserDb>, TempDir) {
    _ = set_global_default(
        registry()
            .with(EnvFilter::new("debug"))
            .with(layer().with_test_writer()),
    );

    let db_dir = tempdir().unwrap();

    let mail_stash = Stash::new(StashConfiguration::test_with_path(
        &db_dir.path().join("test"),
    ))
    .unwrap();

    migrate_core_db(&mail_stash).await.unwrap();
    migrate_mail_db(&mail_stash).await.unwrap();

    (mail_stash, db_dir)
}
