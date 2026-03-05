use core_telemetry::TelemetryEvent;
use std::collections::HashMap;
use uuid::Uuid;

pub struct LatencyEvents;

impl LatencyEvents {
    pub async fn measure_latency<T, E>(
        measurement_group: &str,
        event_name: &str,
        f: impl AsyncFnOnce() -> Result<T, E> + Send,
    ) -> (Result<T, E>, TelemetryEvent)
    where
        E: ToString,
    {
        let start_time_ms = now_unix_ms();

        let result = f().await;
        let error = result.as_ref().err().map(ToString::to_string);

        let event = Self::build_latency_event(measurement_group, event_name, start_time_ms, error);
        (result, event)
    }

    #[must_use]
    fn build_latency_event(
        measurement_group: &str,
        event_name: &str,
        start_time_ms: f64,
        error: Option<String>,
    ) -> TelemetryEvent {
        let end_time_ms = now_unix_ms();
        let status = if error.is_none() { "success" } else { "error" };

        let mut dimensions = HashMap::from([("status".to_string(), status.to_string())]);
        if let Some(msg) = error {
            dimensions.insert("error".to_string(), msg);
        }

        TelemetryEvent {
            id: Uuid::new_v4().to_string(),
            measurement_group: measurement_group.to_string(),
            event: event_name.to_string(),
            values: HashMap::from([
                ("start_time".to_string(), start_time_ms),
                ("end_time".to_string(), end_time_ms),
            ]),
            dimensions,
        }
    }
}

#[must_use]
fn now_unix_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.0
}
