//! Common types used by the Proton API.

pub use mail_api_shared::ApiErrorInfo;

use serde::Deserialize;
use std::time::Duration;

/// Defines timeout values.
pub struct Timeouts;

impl Timeouts {
    pub const QUARTER_SECOND: Duration = Duration::from_millis(250);
    pub const ONE_SECOND: Duration = Duration::from_secs(1);
    pub const TWO_SECONDS: Duration = Duration::from_secs(2);
    pub const QUARTER_MINUTE: Duration = Duration::from_secs(15);
    pub const ONE_MINUTE: Duration = Duration::from_secs(60);
}

pub fn deserialize_bool_from_string<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: String = Deserialize::deserialize(deserializer)?;
    match value.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(serde::de::Error::custom(format!(
            "expected \"true\" or \"false\", found \"{value}\"",
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    struct TestStruct {
        #[serde(deserialize_with = "deserialize_bool_from_string")]
        value: bool,
    }

    #[test]
    fn test_deserialize_bool_from_string_true() {
        let json = r#"{"value": "true"}"#;
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert!(result.value);
    }

    #[test]
    fn test_deserialize_bool_from_string_false() {
        let json = r#"{"value": "false"}"#;
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert!(!result.value);
    }

    #[test]
    fn test_deserialize_bool_from_string_invalid() {
        let json = r#"{"value": "invalid"}"#;
        let result: Result<TestStruct, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("expected \"true\" or \"false\"")
        );
    }
}
