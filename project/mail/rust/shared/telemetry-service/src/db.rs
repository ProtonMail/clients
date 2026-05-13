use anyhow::anyhow;
use async_trait::async_trait;

use core_telemetry::{TelemetryDbEx, TelemetryError, TelemetryEvent};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::params;
use mail_stash::stash::{Stash, StashError};
use mail_stash::utils::MapToSql as _;

use crate::TelemetryEventRow;

pub struct TelemetryDb {
    stash: Stash<UserDb>,
}

impl TelemetryDb {
    #[must_use]
    pub fn new(stash: Stash<UserDb>) -> Self {
        Self { stash }
    }
}

#[async_trait]
impl TelemetryDbEx for TelemetryDb {
    async fn get_events(&self, limit: u32) -> core_telemetry::Result<Vec<TelemetryEvent>> {
        let tether = self.stash.connection();

        let rows = TelemetryEventRow::find("LIMIT ?", params![limit], &tether)
            .await
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let mut event: TelemetryEvent = serde_json::from_str(&row.event_data)
                .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;
            event.id = row.id;
            events.push(event);
        }

        Ok(events)
    }

    async fn insert_events(&self, events: Vec<TelemetryEvent>) -> core_telemetry::Result<()> {
        let mut tether = self.stash.connection();

        tether
            .write_tx(async |tx| {
                for event in events {
                    let id = event.id.clone();
                    let event_data = serde_json::to_string(&event).map_err(|e| {
                        StashError::Custom(anyhow!("Failed to serialize event: {e}"))
                    })?;
                    let mut row = TelemetryEventRow { id, event_data };
                    row.save(tx).await?;
                }
                Ok::<(), StashError>(())
            })
            .await
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })
    }

    async fn delete_events(&self, event_ids: Vec<String>) -> core_telemetry::Result<()> {
        if event_ids.is_empty() {
            return Ok(());
        }

        let mut tether = self.stash.connection();

        tether
            .write_tx(async |tx| {
                let query = format!(
                    "DELETE FROM {} WHERE {} IN ({})",
                    TelemetryEventRow::table_name(),
                    TelemetryEventRow::id_field_name(),
                    mail_stash::utils::placeholders(&event_ids)
                );
                tx.execute(query, event_ids.to_sql()).await?;
                Ok::<(), StashError>(())
            })
            .await
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        Ok(())
    }
}
