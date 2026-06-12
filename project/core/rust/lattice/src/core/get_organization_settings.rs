use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCoreGetOrganizationsSettingsReq;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetOrganizationsSettingsRes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_name: Option<bool>,

    #[serde(rename = "LogoID")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_scribe_writing_assistant: Option<bool>,
}

impl LtContract for LtCoreGetOrganizationsSettingsReq {
    type Response = LtSlimAPIJSON<LtCoreGetOrganizationsSettingsRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/organizations/settings"))
    }
}

impl AuthReq for LtCoreGetOrganizationsSettingsReq {}
