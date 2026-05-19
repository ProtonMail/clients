use async_lock::Mutex;
use async_trait::async_trait;
use rusqlite::{Connection, params};
use std::path::PathBuf;

use crate::{Result, TelemetryDbEx, TelemetryError, TelemetryEvent, queries};

pub struct SqliteDatabase {
    conn: Mutex<Connection>,
}

impl SqliteDatabase {
    pub fn new(storage_path: &str) -> Result<Self> {
        let storage_path = PathBuf::from(storage_path);

        // Ensure the directory exists
        if let Some(parent) = storage_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| TelemetryError::Io { msg: e.to_string() })?;
        }

        // Initialize SQLite database
        let conn = Connection::open(&storage_path)
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        // Create the events table
        conn.execute(queries::CREATE_EVENTS_TABLE, [])
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

#[async_trait]
impl TelemetryDbEx for SqliteDatabase {
    async fn get_events(&self, limit: u32) -> Result<Vec<TelemetryEvent>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn
            .prepare(queries::GET_EVENTS)
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        let mut rows = stmt
            .query(params![limit])
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        let mut events = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?
        {
            let values_bytes: Vec<u8> = row
                .get(3)
                .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;
            let values = serde_json::from_slice(&values_bytes)
                .map_err(|e| TelemetryError::Storage { msg: e.to_string() })?;

            let dimensions_bytes: Vec<u8> = row
                .get(4)
                .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;
            let dimensions = serde_json::from_slice(&dimensions_bytes)
                .map_err(|e| TelemetryError::Storage { msg: e.to_string() })?;

            events.push(TelemetryEvent {
                id: row
                    .get(0)
                    .map_err(|e| TelemetryError::Database { msg: e.to_string() })?,
                measurement_group: row
                    .get(1)
                    .map_err(|e| TelemetryError::Database { msg: e.to_string() })?,
                event: row
                    .get(2)
                    .map_err(|e| TelemetryError::Database { msg: e.to_string() })?,
                values,
                dimensions,
            });
        }

        Ok(events)
    }

    async fn insert_events(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().await;

        let tx = conn
            .unchecked_transaction()
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        // prepared statement
        let mut stmt = tx
            .prepare(queries::INSERT_EVENT)
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        let mut values_buffer = Vec::with_capacity(256);
        let mut dimensions_buffer = Vec::with_capacity(256);

        for event in events {
            // Serialize values
            values_buffer.clear();
            serde_json::to_writer(&mut values_buffer, &event.values).map_err(|e| {
                TelemetryError::Storage {
                    msg: format!("Failed to serialize values: {e}"),
                }
            })?;

            // Serialize dimensions
            dimensions_buffer.clear();
            serde_json::to_writer(&mut dimensions_buffer, &event.dimensions).map_err(|e| {
                TelemetryError::Storage {
                    msg: format!("Failed to serialize dimensions: {e}"),
                }
            })?;

            stmt.execute(params![
                &event.id,
                &event.measurement_group,
                &event.event,
                &values_buffer[..],
                &dimensions_buffer[..],
            ])
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;
        }

        // frees tx
        drop(stmt);

        tx.commit()
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        Ok(())
    }

    async fn delete_events(&self, event_ids: Vec<String>) -> Result<()> {
        if event_ids.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().await;

        let tx = conn
            .unchecked_transaction()
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        let mut stmt = tx
            .prepare(queries::DELETE_EVENT)
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        for id in &event_ids {
            stmt.execute([id])
                .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;
        }

        drop(stmt);
        tx.commit()
            .map_err(|e| TelemetryError::Database { msg: e.to_string() })?;

        Ok(())
    }
}
