use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtQuarkUserResetRes {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    pub password: String,
    #[serde(rename = "UserDecID")]
    pub dec_id: u64,
}
