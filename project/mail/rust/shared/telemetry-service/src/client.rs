use std::sync::Arc;
use std::time::Duration;

use core_telemetry::{SyncedEvents, Tcl, TelemetryError, TelemetryEvent};
use mail_core_api::session::Session;
use mail_sqlite3::MigratorError;
use mail_stash::UserDb;
use mail_stash::params;
use mail_stash::stash::Stash;
use tokio::time::interval_at;
use tracing::{debug, error, info, trace};

use crate::api::TelemetryHttp;
use crate::db::TelemetryDb;
use crate::migration::migrate_user_db;

pub const TELEMETRY_BATCH_SIZE: u32 = 500;
const TELEMETRY_SYNC_INTERVAL_SECS: u64 = 30;

pub struct TelemetryService {
    tcl: Arc<Tcl<TelemetryHttp, TelemetryDb>>,
    session: Session,
    stash: Stash<UserDb>,
}

impl Clone for TelemetryService {
    fn clone(&self) -> Self {
        Self {
            tcl: self.tcl.clone(),
            session: self.session.clone(),
            stash: self.stash.clone(),
        }
    }
}

impl TelemetryService {
    pub async fn new(session: Session, stash: Stash<UserDb>) -> Result<Self, MigratorError> {
        let db = TelemetryDb::new(stash.clone());
        let http = TelemetryHttp::new(session.clone());
        let tcl = Tcl::new(http, db);
        migrate_user_db(&stash).await?;
        Ok(Self {
            tcl: Arc::new(tcl),
            session,
            stash,
        })
    }

    async fn telemetry_enabled(&self) -> Result<bool, String> {
        let tether = self.stash.connection().await.map_err(|e| e.to_string())?;

        let telemetry_enabled = tether
            .query_value_opt::<i64>("SELECT telemetry FROM user_settings LIMIT 1", params![])
            .await
            .map_err(|e| e.to_string())?;

        Ok(matches!(telemetry_enabled, Some(value) if value != 0))
    }

    pub async fn record_event(&self, event: TelemetryEvent) -> core_telemetry::Result<()> {
        if !self
            .telemetry_enabled()
            .await
            .map_err(|e| TelemetryError::Sync { msg: e })?
        {
            return Ok(());
        }

        info!(
            group = %event.measurement_group,
            event_name = %event.event,
            values = ?event.values,
            dimensions = ?event.dimensions,
            "Telemetry: storing event in DB"
        );

        self.tcl.store_events(vec![event]).await
    }

    pub async fn periodic_sync_task(self) {
        let period = Duration::from_secs(TELEMETRY_SYNC_INTERVAL_SECS);
        let start = tokio::time::Instant::now() + period;
        let mut interval = interval_at(start, period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        info!("TelemetryService: background sync task started");

        loop {
            interval.tick().await;

            let telemetry_enabled = match self.telemetry_enabled().await {
                Ok(settings) => settings,
                Err(e) => {
                    error!("TelemetryService: Failed to get user settings: {e}");
                    continue;
                }
            };

            if !telemetry_enabled {
                trace!("Telemetry disabled, skipping telemetry sync");
                continue;
            }

            if !self.session.network_status_observer().is_online() {
                trace!("Network offline, skipping telemetry sync");
                continue;
            }

            match self.tcl.publish_events(TELEMETRY_BATCH_SIZE).await {
                Ok(SyncedEvents::Finished(count)) => {
                    if count > 0 {
                        info!("Telemetry sync: sent {count} events (all done)");
                    } else {
                        info!("Telemetry sync: no events to send");
                    }
                }
                Ok(SyncedEvents::Continue(count)) => {
                    debug!("Telemetry sync: sent {count} events, more remain");
                }
                Err(e) => {
                    error!("Telemetry sync failed: {e:?}");
                }
            }
        }
    }
}
