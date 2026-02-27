//! Database migrations for mail-search crate
//!
//! This module defines migrations for search-related database tables.
//!
//! ## Usage
//!
//! Migrations are automatically run when `MailSearchService::new()` is called.
//! Applications do not need to call these migrations directly - they are handled
//! internally by the mail-search crate during service initialization.
//!
//! ## Migration Versioning
//!
//! Mail-search uses a separate migration version table (`proton_mail_search_version`)
//! which allows it to maintain its own migration numbering sequence
//! independent of mail-common's migrations. This prevents version conflicts and maintains isolation.

use include_dir::{Dir, include_dir};
use mail_sqlite3::{Migrator, MigratorError, file::embedded_migrations};
use mail_stash::{UserDb, stash::Stash};

/// Run search-related database migrations
///
/// This function is called internally by `MailSearchService::new()`.
/// Applications should not call this directly - it's automatically handled
/// during service initialization.
pub(crate) async fn run(mail_stash: &Stash<UserDb>) -> Result<usize, MigratorError> {
    const TABLE: &str = "proton_mail_search_version";
    const MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/migrations");

    let migrations = embedded_migrations(&MIGRATIONS);
    let mut tether = mail_stash.connection().await?;

    Migrator::new(TABLE, migrations).migrate(&mut tether).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_stash::stash::StashConfiguration;

    #[tokio::test]
    async fn smoke() {
        let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
        run(&mail_stash).await.unwrap();
    }
}
