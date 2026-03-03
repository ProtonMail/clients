use include_dir::{Dir, include_dir};
use mail_sqlite3::{Migrator, MigratorError, file::embedded_migrations};
use mail_stash::UserDb;
use mail_stash::stash::Stash;

pub async fn migrate_user_db(stash: &Stash<UserDb>) -> Result<usize, MigratorError> {
    const TABLE: &str = "proton_mail_telemetry_version";
    const MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/migrations");

    Migrator::new(TABLE, embedded_migrations::<UserDb>(&MIGRATIONS))
        .migrate(&mut stash.connection().await?)
        .await
}
