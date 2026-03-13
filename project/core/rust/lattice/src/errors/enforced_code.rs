#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EnforcedCode<const CODE: u32>;

impl<const CODE: u32> std::fmt::Debug for EnforcedCode<CODE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", CODE)
    }
}

impl<const CODE: u32> std::fmt::Display for EnforcedCode<CODE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", CODE)
    }
}

#[cfg(feature = "serde")]
impl<'de, const CODE: u32> serde::Deserialize<'de> for EnforcedCode<CODE> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let code = u32::deserialize(deserializer)?;
        if code == CODE {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(format!("{code} != {CODE}")))
        }
    }
}

#[cfg(feature = "serde")]
impl<const CODE: u32> serde::Serialize for EnforcedCode<CODE> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(CODE)
    }
}

impl<const CODE: u32> std::default::Default for EnforcedCode<CODE> {
    fn default() -> Self {
        Self
    }
}
