use include_dir::{Dir, include_dir};
use mail_sqlite3::file::embedded_migrations;
use mail_sqlite3::{Migrator, MigratorError};
use mail_stash::UserDb;
use mail_stash::stash::Tether;

pub async fn migrate(conn: &mut Tether<UserDb>) -> Result<(), MigratorError> {
    const TABLE: &str = "labels_version";
    const MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/db/migrations");
    Migrator::new(TABLE, embedded_migrations::<UserDb>(&MIGRATIONS))
        .migrate(conn)
        .await?;
    Ok(())
}
