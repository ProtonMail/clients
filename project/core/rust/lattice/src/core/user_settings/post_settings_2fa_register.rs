use std::borrow::Cow;

use passkey::types::webauthn::{AuthenticatorTransport, CredentialCreationOptions};

use super::LtCoreUserSettings;
use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePostSettings2faRegisterReq {
    pub name: String,

    pub client_data: Sensitive<String>,
    pub attestation_object: Sensitive<String>,
    pub transports: Vec<AuthenticatorTransport>,

    pub registration_options: CredentialCreationOptions,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePostSettings2faRegisterRes {
    pub user_settings: LtCoreUserSettings,
}

impl LtContract for LtCorePostSettings2faRegisterReq {
    type Response = LtSlimAPIJSON<LtCorePostSettings2faRegisterRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/settings/2fa/register"))
    }
}

impl AuthReq for LtCorePostSettings2faRegisterReq {}
