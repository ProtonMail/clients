use async_trait::async_trait;
use core_telemetry::{TelemetryError, TelemetryEvent, TelemetryHttpClientEx};
use mail_core_api::services::proton::DATA_V1;
use mail_core_api::session::Session;
use mail_muon::POST;
use mail_muon::common::ServiceType;
use mail_muon::util::ProtonRequestExt;
use serde_json::json;

pub struct TelemetryHttp {
    session: Session,
}

impl TelemetryHttp {
    #[must_use]
    pub fn new(session: Session) -> Self {
        Self { session }
    }
}

#[async_trait]
impl TelemetryHttpClientEx for TelemetryHttp {
    async fn send(&self, events: Vec<TelemetryEvent>) -> core_telemetry::Result<()> {
        POST!("{DATA_V1}/stats/multiple")
            .body_json(json!({ "EventInfo": events }))
            .map_err(|e| TelemetryError::Sync { msg: e.to_string() })?
            .header(("Priority", "u=6"))
            .service_type(ServiceType::Background, true)
            .send_with(&self.session)
            .await
            .map_err(|e| TelemetryError::Sync { msg: e.to_string() })?
            .ok()
            .map_err(|e| TelemetryError::Sync { msg: e.to_string() })?;

        Ok(())
    }
}
