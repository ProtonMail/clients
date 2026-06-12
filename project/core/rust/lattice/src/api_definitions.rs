use derive_more::Display;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtApiResponse<T> {
    pub code: LtApiCode,

    #[serde(flatten)]
    pub body: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, Deserialize, Serialize)]
pub struct LtApiCode(pub u32);

impl LtApiCode {
    pub const OK: Self = Self(1000);
}
