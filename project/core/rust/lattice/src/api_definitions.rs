use derive_more::Display;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtApiResponse<T> {
    pub code: LtApiCode,

    #[cfg_attr(feature = "serde", serde(flatten))]
    pub body: T,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
pub struct LtApiCode(pub u32);

impl LtApiCode {
    pub const OK: Self = Self(1000);
}
