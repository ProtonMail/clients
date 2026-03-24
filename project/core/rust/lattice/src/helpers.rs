/// A module for serializing and deserializing `bool` as an `i32`.
///
/// The `deserialize` function converts `true` to `1` and `false` to `0`,
/// yielding an error if the value is not `1` or `0`.
#[cfg(feature = "serde")]
pub mod bool_int {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &bool, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert true -> 1, false -> 0
        s.serialize_i32(if *value { 1 } else { 0 })
    }

    pub fn deserialize<'de, D>(d: D) -> Result<bool, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize the input as an i32 first
        let val = i32::deserialize(d)?;
        match val {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err(serde::de::Error::custom(format!(
                "invalid boolean integer: {val}, expected 0 or 1"
            ))),
        }
    }
}

/// A module for serializing and deserializing `Option<bool>` as an `i32`.
///
/// The `deserialize` function converts `1` to `Some(true)` and `0` to `Some(false)`,
/// None if the value is not present,
/// yielding an error if the value is present but not `1` or `0`.
#[cfg(feature = "serde")]
#[cfg(feature = "auth")]
pub mod bool_opt_int {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<bool>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) => s.serialize_some(&(*v as i32)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<bool>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<i32>::deserialize(d)?;
        match opt {
            Some(1) => Ok(Some(true)),
            Some(0) => Ok(Some(false)),
            None => Ok(None),
            _ => Err(serde::de::Error::custom("invalid bool int")),
        }
    }
}
