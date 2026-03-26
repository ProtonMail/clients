//! Unleash API data types.
//!
//! These types mirror the Unleash frontend features API response structure.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unleash API base path (v2).
pub const UNLEASH_V2: &str = "/feature/v2";

/// A single feature toggle from the Unleash API.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize, Default))]
#[serde(rename_all = "camelCase")]
pub struct UnleashToggle {
    pub name: String,
    /// According to the Unleash API, always true.
    /// <https://docs.getunleash.io/reference/api/unleash/get-frontend-features/>
    ///
    /// See: [`UnleashToggleVariant::feature_enabled`]
    pub enabled: bool,
    /// `true` if the impression data collection is enabled for the feature.
    pub impression_data: bool,
    pub variant: UnleashToggleVariant,
}

/// Variant of an Unleash toggle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize, Default))]
// Yes, Unleash API is inconsistent in naming fields
#[serde(rename_all = "snake_case")]
pub struct UnleashToggleVariant {
    pub name: String,
    #[serde(default = "default_feature_enabled")]
    pub feature_enabled: bool,
    #[serde(default)]
    pub payload: Option<UnleashTogglePayload>,
}

fn default_feature_enabled() -> bool {
    // Educated guess: If `feature_enabled` is missing, it means the feature is enabled.
    true
}

/// Payload attached to an Unleash toggle variant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize, Default))]
pub struct UnleashTogglePayload {
    #[serde(rename = "type")]
    pub ty: UnleashTogglePayloadType,
    pub value: String,
}

/// Type of payload in an Unleash toggle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize, Default))]
#[serde(rename_all = "snake_case")]
pub enum UnleashTogglePayloadType {
    Json,
    Csv,
    #[cfg_attr(feature = "mocks", default)]
    String,
    Number,
}

/// Response from the Unleash frontend features API.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "camelCase")]
pub struct GetUnleashFeaturesResponse {
    pub toggles: Vec<UnleashToggle>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetUnleashFeaturesRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<GetUnleashFeaturesContext>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetUnleashFeaturesContext {
    /// The name of the application, >=1 character if present
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// A DateTime or similar data class instance or a string in a RFC3339-compatible
    /// format. Defaults to the current time if not set by the user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_time: Option<String>,
    /// Additional Unleash context properties
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, String>,
    /// The app's IP address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_address: Option<String>,
    /// An identifier for the current session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// An identifier for the current user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}
