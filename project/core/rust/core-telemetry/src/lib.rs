pub mod client;
#[cfg(feature = "http")]
pub mod http;
#[cfg(feature = "sqlite")]
mod queries;
#[cfg(feature = "sqlite")]
mod storage;

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, TelemetryError>;

#[cfg_attr(feature = "uniffi", uniffi::export(with_foreign))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait TelemetryHttpClientEx: Send + Sync {
    async fn send(&self, events: Vec<TelemetryEvent>) -> Result<()>;
}

#[cfg_attr(feature = "uniffi", uniffi::export(with_foreign))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait TelemetryDbEx: Send + Sync {
    async fn get_events(&self, limit: u32) -> Result<Vec<TelemetryEvent>>;
    async fn insert_events(&self, events: Vec<TelemetryEvent>) -> Result<()>;
    async fn delete_events(&self, event_ids: Vec<String>) -> Result<()>;
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

#[derive(Debug, Clone)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct TelemetryConfig {
    pub storage_path: String,
    pub max_storage_size: u64,
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
