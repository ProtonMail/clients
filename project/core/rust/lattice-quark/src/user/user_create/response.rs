use serde::{Deserialize, Serialize};

use super::super::LtQuarkUserStatus;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtQuarkUserCreateRes {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    pub password: String,
    pub status: LtQuarkUserStatus,
    pub recovery: String,
    pub recovery_verified: u8,
    pub recovery_phone: String,
    pub auth_version: u8,
    #[serde(rename = "Created at")]
    pub created_at: String,
    #[serde(rename = "Dec_ID")]
    pub dec_id: u64,
    pub status_info: String,
    pub mailbox_password: Option<String>,
}
