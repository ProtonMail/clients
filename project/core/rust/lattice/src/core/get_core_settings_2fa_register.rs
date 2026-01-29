use std::borrow::Cow;

use crate::{AuthReq, LatticeContract, LatticeError, auth::LtAuthFidoKey};
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

impl LatticeContract for LtCoreGetSettings2faRegisterReq {
    type Response = LtCoreGetSettings2faRegisterRes;
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/settings/2fa/register"))
    }
}

impl AuthReq for LtCoreGetSettings2faRegisterReq {}
