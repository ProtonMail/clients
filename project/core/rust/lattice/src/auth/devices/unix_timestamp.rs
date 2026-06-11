//! Unix-second timestamps on auth-device payloads.
//!
//! Values are always interpreted as Unix seconds. Symfony may emit JSON strings instead of numbers.

use derive_more::{Deref, From, Into};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deref, From, Into
)]
pub struct LtUnixTimestamp(pub i64);

#[cfg(feature = "serde")]
impl serde::Serialize for LtUnixTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for LtUnixTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Unexpected, Visitor};

        struct UnixTimestampVisitor;

        impl Visitor<'_> for UnixTimestampVisitor {
            type Value = LtUnixTimestamp;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a unix timestamp as i64 or decimal string")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(LtUnixTimestamp(value))
            }

            fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_i64(i64::from(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                i64::try_from(value)
                    .map(LtUnixTimestamp)
                    .map_err(|_| de::Error::invalid_value(Unexpected::Unsigned(value), &self))
            }

            fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_i64(i64::from(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                value
                    .parse()
                    .map(LtUnixTimestamp)
                    .map_err(|_| de::Error::invalid_value(Unexpected::Str(value), &self))
            }
        }

        deserializer.deserialize_any(UnixTimestampVisitor)
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::{
        auth::devices::{LtAuthDevice, LtAuthDeviceState},
        core::LtCoreAuthDeviceId,
    };
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct DeviceTimes {
        create_time: LtUnixTimestamp,
        #[serde(default)]
        activate_time: Option<LtUnixTimestamp>,
    }

    #[test]
    fn deserializes_string_timestamps() {
        let v: DeviceTimes =
            serde_json::from_str(r#"{"create_time":"1710000000","activate_time":null}"#).unwrap();
        assert_eq!(i64::from(v.create_time), 1_710_000_000);
        assert_eq!(v.activate_time, None);
    }

    #[test]
    fn deserializes_numeric_timestamps() {
        let v: DeviceTimes =
            serde_json::from_str(r#"{"create_time":1710000000,"activate_time":"1710003600"}"#)
                .unwrap();
        assert_eq!(i64::from(v.create_time), 1_710_000_000);
        assert_eq!(v.activate_time.map(i64::from), Some(1_710_003_600));
    }

    #[test]
    fn deserializes_lt_auth_device_symfony_shapes() {
        let device: LtAuthDevice = serde_json::from_str(
            r#"{
                "ID": "device-1",
                "State": 1,
                "Name": "test-device",
                "LocalizedClientName": "Test Client",
                "CreateTime": "1710000000",
                "LastActivityTime": 1710003600,
                "ActivateTime": "1710001800"
            }"#,
        )
        .unwrap();

        assert_eq!(device.id, LtCoreAuthDeviceId("device-1".to_string()));
        assert_eq!(device.state, LtAuthDeviceState::Active);
        assert_eq!(i64::from(device.create_time), 1_710_000_000);
        assert_eq!(i64::from(device.last_activity_time), 1_710_003_600);
        assert_eq!(device.activate_time.map(i64::from), Some(1_710_001_800));

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["CreateTime"], 1_710_000_000);
        assert_eq!(json["LastActivityTime"], 1_710_003_600);
        assert_eq!(json["ActivateTime"], 1_710_001_800);
    }
}
