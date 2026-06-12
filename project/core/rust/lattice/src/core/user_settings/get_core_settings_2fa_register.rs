use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, auth::LtAuthFidoKey,
};
use passkey::types::webauthn::{AttestationStatementFormatIdentifiers, CredentialCreationOptions};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetSettings2faRegisterRes {
    pub registration_options: CredentialCreationOptions,
    pub attestation_formats: Vec<AttestationStatementFormatIdentifiers>,
    pub registered_keys: Vec<LtAuthFidoKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCoreGetSettings2faRegisterReq;

impl LtContract for LtCoreGetSettings2faRegisterReq {
    type Response = LtSlimAPIJSON<LtCoreGetSettings2faRegisterRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/settings/2fa/register"))
    }
}

impl AuthReq for LtCoreGetSettings2faRegisterReq {}
