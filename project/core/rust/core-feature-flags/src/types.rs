//! Unleash API data types.
//!
//! These types mirror the Unleash frontend features API response structure.

use serde::Deserialize;
#[cfg(feature = "mocks")]
use serde::Serialize;

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
