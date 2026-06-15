use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::core::user::LtCoreSrpVerifier;
use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

/// Request body for `PUT /core/v4/settings/password`.
///
/// Two-pass accounts call this to rotate the login (primary) password only:
/// the new SRP verifier replaces the server-side one, and private keys remain
/// encrypted with the unchanged mailbox (secondary) passphrase.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePutSettingsPasswordReq {
    pub auth: LtCoreSrpVerifier,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePutSettingsPasswordRes {}

impl LtContract for LtCorePutSettingsPasswordReq {
    type Response = LtSlimAPIJSON<LtCorePutSettingsPasswordRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/settings/password"))
    }
}

impl AuthReq for LtCorePutSettingsPasswordReq {}
