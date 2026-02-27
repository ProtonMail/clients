pub mod offline_migrations;
pub mod online_migrations;

pub type DBMigrationError = MigratorError;

pub use mail_sqlite3;

use mail_sqlite3::migration::MigratorError;
