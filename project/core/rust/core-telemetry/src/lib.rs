pub mod client;
#[cfg(feature = "uniffi")]
pub mod ffi;
#[cfg(feature = "http")]
pub mod http;
#[cfg(feature = "sqlite")]
mod queries;
#[cfg(feature = "sqlite")]
pub mod storage;

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();

pub use client::Tcl;

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, TelemetryError>;

#[async_trait]
pub trait TelemetryHttpClientEx: Send {
    async fn send(&self, events: Vec<TelemetryEvent>) -> Result<()>;
}

#[async_trait]
impl<T: TelemetryHttpClientEx + Sync> TelemetryHttpClientEx for &T {
    #[inline]
    async fn send(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        (*self).send(events).await
    }
}

#[async_trait]
pub trait TelemetryDbEx: Send {
    async fn get_events(&self, limit: u32) -> Result<Vec<TelemetryEvent>>;
    async fn insert_events(&self, events: Vec<TelemetryEvent>) -> Result<()>;
    async fn delete_events(&self, event_ids: Vec<String>) -> Result<()>;
}

#[async_trait]
impl<T: TelemetryDbEx + Sync> TelemetryDbEx for &T {
    #[inline]
    async fn get_events(&self, limit: u32) -> Result<Vec<TelemetryEvent>> {
        (*self).get_events(limit).await
    }

    #[inline]
    async fn insert_events(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        (*self).insert_events(events).await
    }

    #[inline]
    async fn delete_events(&self, event_ids: Vec<String>) -> Result<()> {
        (*self).delete_events(event_ids).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct TelemetryEvent {
    #[serde(default, skip_serializing)]
    pub id: String, // only for client-side db, this does not get send to the backend
    pub measurement_group: String,
    pub event: String,
    pub values: HashMap<String, f64>,
    pub dimensions: HashMap<String, String>,
}

#[derive(Error, Debug)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Error))]
pub enum TelemetryError {
    #[error("Storage error: {msg}")]
    Storage { msg: String },
    #[error("Sync failed: {msg}")]
    Sync { msg: String },
    #[error("Configuration error: {msg}")]
    Configuration { msg: String },
    #[error("Database error: {msg}")]
    Database { msg: String },
    #[error("IO error: {msg}")]
    Io { msg: String },
    #[error("Operation cancelled")]
    Cancelled,
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[derive(Debug)]
#[must_use = "If `Continue` you should sync an other batch of events"]
pub enum SyncedEvents {
    /// Finished processing all available telemetry events
    Finished(u64),
    /// Finished processing `amount` of events, and more events
    /// should be synced
    Continue(u64),
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHttp;
    struct MockDb;

    #[async_trait]
    impl TelemetryHttpClientEx for MockHttp {
        async fn send(&self, _events: Vec<TelemetryEvent>) -> Result<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl TelemetryDbEx for MockDb {
        async fn get_events(&self, _limit: u32) -> Result<Vec<TelemetryEvent>> {
            Ok(vec![])
        }
        async fn insert_events(&self, _events: Vec<TelemetryEvent>) -> Result<()> {
            Ok(())
        }
        async fn delete_events(&self, _event_ids: Vec<String>) -> Result<()> {
            Ok(())
        }
    }

    fn dummy_event() -> TelemetryEvent {
        TelemetryEvent {
            id: "123".into(),
            measurement_group: "telemetry.test".into(),
            event: "test_event".into(),
            values: HashMap::new(),
            dimensions: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_tcl_with_borrowed() {
        let http = MockHttp;
        let db = MockDb;

        let tcl1 = Tcl::new(&http, &db);
        tcl1.store_events(vec![dummy_event()]).await.unwrap();

        let tcl2 = Tcl::new(&http, &db);
        tcl2.store_events(vec![dummy_event()]).await.unwrap();
    }

    #[tokio::test]
    async fn test_tcl_with_owned() {
        let tcl = Tcl::new(MockHttp, MockDb);
        tcl.store_events(vec![dummy_event()]).await.unwrap();
    }
}
