use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtQuarkUserCreateOrganizationRes {
    #[serde(rename = "OrganizationID")]
    pub organization_id: u64,
    pub org_pass: String,
    pub org_salt: String,
    pub max_vpn: u32,
    pub max_space: u64,
    pub org_name: String,
}
