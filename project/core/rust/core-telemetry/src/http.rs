use crate::{Result, TelemetryError, TelemetryEvent, TelemetryHttpClientEx};
use async_trait::async_trait;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::json;

const BASE_URL: &str = "https://mail.proton.me/api";

pub struct TelemetryHttpClient {
    client: Client,
}

impl TelemetryHttpClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for TelemetryHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TelemetryHttpClientEx for TelemetryHttpClient {
    async fn send(&self, events: Vec<TelemetryEvent>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let url = format!("{BASE_URL}/data/v1/stats/multiple");
        let payload = json!({ "EventInfo": events });
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            "x-pm-appversion",
            HeaderValue::from_static("web-mail@5.0.60.0"), // unclear what this version is, but
                                                           // it's needed
        );

        let resp = self
            .client
            .post(&url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|e| TelemetryError::Sync { msg: e.to_string() })?;

        if !resp.status().is_success() {
            return Err(TelemetryError::Sync {
                msg: format!("HTTP error: {}", resp.status()),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::TelemetryHttpClient;
    use crate::{TelemetryEvent, TelemetryHttpClientEx};
    use std::collections::HashMap;

    #[tokio::test]
    #[cfg(feature = "http")]
    #[ignore = "This is an integration test that hits a real API, run it manually"]
    async fn test_send_telemetry() {
        let mut values = HashMap::new();
        values.insert("start_time".to_string(), 1008.0);
        values.insert("end_time".to_string(), 2008.0);

        let mut dimensions = HashMap::new();
        dimensions.insert("client_version".to_string(), "1.1.0".to_string());

        let event = TelemetryEvent {
            id: String::new(),
            measurement_group: "any.web.test_client".to_string(),
            event: "test_event_1".to_string(),
            values,
            dimensions,
        };

        let client = TelemetryHttpClient::new();
        client
            .send(vec![event])
            .await
            .expect("Failed to send telemetry");
    }
}
