use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtQuarkUserCreateSubuserRes {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    pub password: String,
    pub auth_version: u8,
    #[serde(rename = "UserDecID")]
    pub user_dec_id: u64,
    pub status_name: String,
    pub status: u8,
}
