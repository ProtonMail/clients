use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, auth::LtAuthFidoKey,
};
use passkey::types::webauthn::{AttestationStatementFormatIdentifiers, CredentialCreationOptions};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetSettings2faRegisterRes {
    pub registration_options: CredentialCreationOptions,
    pub attestation_formats: Vec<AttestationStatementFormatIdentifiers>,
    pub registered_keys: Vec<LtAuthFidoKey>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
