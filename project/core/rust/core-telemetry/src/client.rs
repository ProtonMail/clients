use crate::{
    Result, SyncedEvents, TelemetryDbEx, TelemetryError, TelemetryEvent, TelemetryHttpClientEx,
};

// Maximum batch size for sync operations
const MAX_SYNC_BATCH: u32 = 1_000;

#[derive(Debug)]
pub struct Tcl<Http, Db> {
    http: Http,
    db: Db,
}

impl<Http: TelemetryHttpClientEx, Db: TelemetryDbEx> Tcl<Http, Db> {
    pub const fn new(http: Http, db: Db) -> Self {
        Self { http, db }
    }

    pub async fn store_events(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        self.db.insert_events(events).await
    }

    pub async fn publish_events(&self, batch_size: u32) -> Result<SyncedEvents> {
        let batch_size = batch_size.min(MAX_SYNC_BATCH);
        let events = self.db.get_events(batch_size).await?;

        if events.is_empty() {
            return Ok(SyncedEvents::Finished(0));
        }
        let event_ids: Vec<String> = events.iter().map(|e| e.id.clone()).collect();

        self.http
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
