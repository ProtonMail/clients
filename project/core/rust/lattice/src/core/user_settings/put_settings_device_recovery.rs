use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

use super::LtCoreUserSettings;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePutDeviceRecoveryPreferenceRes {
    pub user_settings: LtCoreUserSettings,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePutDeviceRecoveryPreferenceReq {
    #[cfg_attr(feature = "serde", serde(with = "crate::helpers::bool_int"))]
    pub device_recovery: bool,
}

impl LtContract for LtCorePutDeviceRecoveryPreferenceReq {
    type Response = LtSlimAPIJSON<LtCorePutDeviceRecoveryPreferenceRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/settings/devicerecovery"))
    }
}

impl AuthReq for LtCorePutDeviceRecoveryPreferenceReq {}
