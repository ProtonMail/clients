use crate::db::migrations::migrate_core_db;
use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashConfiguration};
use tracing::subscriber::set_global_default;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, registry};

pub async fn new_core_test_connection() -> Stash<UserDb> {
    _ = set_global_default(
        registry()
            .with(EnvFilter::new("debug"))
            .with(layer().with_test_writer()),
    );

    let mail_stash = Stash::new(StashConfiguration::test()).unwrap();

    migrate_core_db(&mail_stash).await.unwrap();

    mail_stash
}
