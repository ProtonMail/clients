use serde::Deserialize;
#[cfg(feature = "mocks")]
use serde::Serialize;
use serde_json::{Error as JsonError, Value as JsonValue};

/// Information for the human verification challenge.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct HumanVerificationChallenge {
    pub description: String,
    pub direct: u8,
    pub expires_at: u64,
    pub human_verification_methods: Vec<String>,
    pub human_verification_token: String,
    pub web_url: String,
}

impl HumanVerificationChallenge {
    pub fn from_value(value: JsonValue) -> Result<Self, JsonError> {
        serde_json::from_value(value)
    }
}
