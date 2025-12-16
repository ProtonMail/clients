use async_lock::Mutex;
use std::sync::Arc;

use crate::{
    Result, SyncedEvents, TelemetryConfig, TelemetryDbEx, TelemetryError, TelemetryEvent,
    TelemetryHttpClientEx,
};

#[cfg(feature = "http")]
use crate::http::TelemetryHttpClient;
#[cfg(feature = "sqlite")]
use crate::storage::SqliteDatabase;

// Maximum batch size for sync operations
const MAX_SYNC_BATCH: u32 = 1_000;

#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct Tcl {
    _config: TelemetryConfig,
    http_client: Arc<dyn TelemetryHttpClientEx>,
    db: Arc<dyn TelemetryDbEx>,
    sync_lock: Mutex<()>,
}

/// Both features enabled - uses default HTTP client and SQLite database provided by this library
#[cfg(all(feature = "http", feature = "sqlite"))]
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Tcl {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn init(config: TelemetryConfig) -> Result<Arc<Self>> {
        let http_client = Arc::new(TelemetryHttpClient::new());
        let db = Arc::new(
            SqliteDatabase::new(&config.storage_path)
                .map_err(|e| TelemetryError::Database { msg: e.to_string() })?,
        );

        Ok(Arc::new(Self {
            _config: config,
            http_client,
            db,
            sync_lock: Mutex::new(()),
        }))
    }
}

/// Only sqlite feature enabled - must provide own http client implementation
#[cfg(all(feature = "sqlite", not(feature = "http")))]
impl Tcl {
    pub fn init(
        config: TelemetryConfig,
        http_client: Arc<dyn TelemetryHttpClientEx>,
    ) -> Result<Arc<Self>> {
        let db = Arc::new(
            SqliteDatabase::new(&config.storage_path)
                .map_err(|e| TelemetryError::Database { msg: e.to_string() })?,
        );

        Ok(Arc::new(Self {
            _config: config,
            http_client,
            db,
            sync_lock: Mutex::new(()),
        }))
    }
}

/// Only http feature enabled - must provide own database implementation
#[cfg(all(feature = "http", not(feature = "sqlite")))]
impl Tcl {
    pub fn init(config: TelemetryConfig, db: Arc<dyn TelemetryDbEx>) -> Result<Arc<Self>> {
        let http_client = Arc::new(TelemetryHttpClient::new());

        Ok(Arc::new(Self {
            _config: config,
            http_client,
            db,
            sync_lock: Mutex::new(()),
        }))
    }
}

/// Neither feature enabled - must provide both HTTP client and database implementations
#[cfg(not(any(feature = "http", feature = "sqlite")))]
impl Tcl {
    pub fn init(
        config: TelemetryConfig,
        http_client: Arc<dyn TelemetryHttpClientEx>,
        db: Arc<dyn TelemetryDbEx>,
    ) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            _config: config,
            http_client,
            db,
            sync_lock: Mutex::new(()),
        }))
    }
}

impl Tcl {
    #[cfg(feature = "uniffi")]
    #[uniffi::constructor]
    pub fn new(
        _config: TelemetryConfig,
        http_client: Arc<dyn TelemetryHttpClientEx>,
        db: Arc<dyn TelemetryDbEx>,
    ) -> Arc<Self> {
        Arc::new(Self {
            _config,
            http_client,
            db,
            sync_lock: Mutex::new(()),
        })
    }

    async fn log_impl(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        self.db.insert_events(events).await
    }

    async fn sync_impl(&self, batch_size: u32) -> Result<SyncedEvents> {
        let batch_size = batch_size.min(MAX_SYNC_BATCH);

        let events = self.db.get_events(batch_size).await?;

        if events.is_empty() {
            return Ok(SyncedEvents::Finished(0));
        }

        let event_ids: Vec<String> = events.iter().map(|e| e.id.clone()).collect();

        self.http_client
            .send(events)
            .await
            .map_err(|e| TelemetryError::Sync {
                msg: format!("Failed to send telemetry: {e:?}"),
            })?;

        let amount = event_ids.len() as u64;
        self.db.delete_events(event_ids).await?;

        if amount < batch_size as u64 {
            Ok(SyncedEvents::Finished(amount))
        } else {
            Ok(SyncedEvents::Continue(amount))
        }
    }
}

#[cfg(feature = "uniffi")]
#[uniffi::export]
impl Tcl {}

#[cfg_attr(all(feature = "uniffi", not(target_family = "wasm")), uniffi::export)]
#[cfg(not(target_family = "wasm"))]
impl Tcl {
    /// Log telemetry events to the database.
    pub async fn log(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        self.log_impl(events).await
    }

    /// Synchronize telemetry events with the remote server.
    ///
    /// Reads events from the database in batches, sends them via the HTTP client,
    /// and deletes successfully sent events from the database.
    pub async fn sync(&self, batch_size: u32) -> Result<SyncedEvents> {
        let _guard = self.sync_lock.lock().await;
        self.sync_impl(batch_size).await
    }
}

#[cfg(target_family = "wasm")]
impl Tcl {
    /// Log telemetry events to the database (Wasm version).
    pub async fn log(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        use send_wrapper::SendWrapper;
        SendWrapper::new(self.log_impl(events)).await
    }

    /// Synchronize telemetry events with the remote server (WASM version).
    ///
    /// Reads events from the database in batches, sends them via the HTTP client,
    /// and deletes successfully sent events from the database.
    pub async fn sync(&self, batch_size: u32) -> Result<SyncedEvents> {
        use send_wrapper::SendWrapper;
        let _guard = self.sync_lock.lock().await;
        SendWrapper::new(self.sync_impl(batch_size)).await
    }
}

impl std::fmt::Debug for Tcl {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Tcl")
    }
}

#[cfg(test)]
mod tests {
    use crate::{TelemetryConfig, TelemetryEvent};

    // cargo test --features "http,sqlite" -- --nocapture
    // cargo test --features "http,sqlite" -- --ignored --nocapture
    // this is more of an integration test
    // tests e2e create event -> store in db -> send it to **REAL** server
    #[tokio::test]
    #[ignore = "This is an integration test, run it manually"]
    #[cfg(all(feature = "sqlite", feature = "http"))]
    async fn test_log_and_sync() {
        use crate::client::Tcl;

        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db_path = temp_file.path().to_str().unwrap();
        let config = TelemetryConfig {
            storage_path: db_path.to_string(),
            max_storage_size: 100,
        };
        let tcl = Tcl::init(config).unwrap();

        let event = TelemetryEvent {
            id: "".to_string(),
            measurement_group: "any.web.test_client".to_string(),
            event: "test_event_1".to_string(),
            values: vec![
                ("start_time".to_string(), 4110.0),
                ("end_time".to_string(), 2000.0),
            ]
            .into_iter()
            .collect(),
            dimensions: vec![("client_version".to_string(), "1.1.1".to_string())]
                .into_iter()
                .collect(),
        };

        tcl.log(vec![event.clone()]).await.unwrap();
        let result = tcl.sync(10).await;
        assert!(result.is_ok());

        println!("Sync Sent");
    }
}
