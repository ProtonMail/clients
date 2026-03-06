mod api;
mod client;
mod db;
mod instruments;
mod migration;
mod model;

pub use client::{TELEMETRY_BATCH_SIZE, TelemetryService};
pub use core_telemetry::{SyncedEvents, TelemetryEvent};
pub use db::TelemetryDb;
pub use instruments::*;
pub use migration::migrate_user_db;
pub use model::TelemetryEventRow;

/// In telemetry dimensions are stored as a `HashMap<String, String>`
/// This is utility trait that should be combined with some kind of derive macro of `Display`.
/// For example `strum::Display` in order to have typesafe representation of those.
pub trait Dimension: ToString + Sized {
    const NAME: &str;

    fn to_dimension(self) -> (String, String) {
        (Self::NAME.to_string(), self.to_string())
    }
}
