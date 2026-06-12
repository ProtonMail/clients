use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use super::LtCoreUserSettings;
use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetSettingsRes {
    pub user_settings: LtCoreUserSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCoreGetSettingsReq;

impl LtContract for LtCoreGetSettingsReq {
    type Response = LtSlimAPIJSON<LtCoreGetSettingsRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/settings"))
    }
}

impl AuthReq for LtCoreGetSettingsReq {}
