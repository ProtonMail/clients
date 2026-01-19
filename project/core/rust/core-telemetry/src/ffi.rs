use crate::{
    Result, SyncedEvents, TelemetryError, TelemetryEvent, client::Tcl, http::TelemetryHttpClient,
    storage::SqliteDatabase,
};

#[derive(uniffi::Object)]
pub struct TclFfi {
    http_client: TelemetryHttpClient,
    db: SqliteDatabase,
    mutex: async_lock::Mutex<()>,
}

#[uniffi::export]
impl TclFfi {
    #[uniffi::constructor]
    pub fn new(storage_path: String) -> Result<Self> {
        let http_client = TelemetryHttpClient::new();
        let db = SqliteDatabase::new(&storage_path)
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        Ok(Self {
            http_client,
            db,
            mutex: async_lock::Mutex::new(()),
        })
    }
}

#[uniffi::export]
impl TclFfi {
    /// Log telemetry events to the database.
    pub async fn store_events(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        Tcl::new(&self.http_client, &self.db)
            .store_events(events)
            .await
    }

    /// Synchronize telemetry events with the remote server.
    ///
    /// Reads events from the database in batches, sends them via the HTTP client,
    /// and deletes successfully sent events from the database.
    pub async fn publish_events(&self, batch_size: u32) -> Result<SyncedEvents> {
        let guard = self.mutex.lock().await;
        let res = Tcl::new(&self.http_client, &self.db)
            .publish_events(batch_size)
            .await;
        drop(guard);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // cargo test --features "http,sqlite" -- --nocapture
    // cargo test --features "http,sqlite" -- --ignored --nocapture
    // this is more of an integration test
    // tests e2e create event -> store in db -> send it to **REAL** server
    #[tokio::test]
    #[ignore = "This is an integration test, run it manually"]
    async fn test_log_and_sync() {
        use crate::TelemetryEvent;

        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db_path = temp_file.path().to_str().unwrap().to_string();
        let tcl = TclFfi::new(db_path).unwrap();

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

        tcl.store_events(vec![event.clone()]).await.unwrap();
        let result = tcl.publish_events(10).await;
        assert!(result.is_ok());

        println!("Sync Sent");
    }
}
