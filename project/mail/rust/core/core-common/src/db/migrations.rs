#[cfg(test)]
#[path = "../tests/db/migrations.rs"]
mod tests;

use include_dir::{Dir, include_dir};
use mail_sqlite3::{Migrator, MigratorError, file::embedded_migrations};
use mail_stash::stash::Stash;
use mail_stash::{AccountDb, UserDb};

fn account_db() -> Migrator<AccountDb> {
    const TABLE: &str = "proton_account_version";
    const MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/db/migrations/account");

    Migrator::new(TABLE, embedded_migrations::<AccountDb>(&MIGRATIONS))
}

pub async fn migrate_account_db(mail_stash: &Stash<AccountDb>) -> Result<usize, MigratorError> {
    account_db()
        .migrate(&mut mail_stash.connection().await?)
        .await
}

pub async fn verify_account_db(mail_stash: &Stash<AccountDb>) -> Result<(), MigratorError> {
    account_db()
        .verify(&mut mail_stash.connection().await?)
        .await
}

fn core_db() -> Migrator<UserDb> {
    const TABLE: &str = "proton_core_version";
    const MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/db/migrations/core");

    Migrator::new(TABLE, embedded_migrations(&MIGRATIONS))
}

pub async fn migrate_core_db(mail_stash: &Stash<UserDb>) -> Result<usize, MigratorError> {
    let mut tether = mail_stash.connection().await?;

    // Create action queue tables first as we can have items that depend on this.
    // This is safe to call multiple times as the migration code guarantees that
    // this will only run once per new version.
    mail_action_queue::db::migrate(&mut tether).await?;

    core_db().migrate(&mut tether).await
}

pub async fn verify_core_db(mail_stash: &Stash<UserDb>) -> Result<(), MigratorError> {
    core_db().verify(&mut mail_stash.connection().await?).await
}
