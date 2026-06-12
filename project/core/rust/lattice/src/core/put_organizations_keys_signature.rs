use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::auth::LtAuthAddressId;
use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive};

/// `PUT /core/v4/organizations/keys/signature` — publish org-identity (fingerprint) signature.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCorePutOrganizationsKeysSignatureReq {
    pub body: LtCorePutOrganizationsKeysSignatureBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePutOrganizationsKeysSignatureBody {
    /// Armored detached PGP signature over the org key SHA-256 fingerprint.
    pub signature: Sensitive<String>,

    /// Encrypted address id of the signing key.
    #[serde(rename = "AddressID")]
    pub address_id: LtAuthAddressId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LtCorePutOrganizationsKeysSignatureRes {}

impl LtContract for LtCorePutOrganizationsKeysSignatureReq {
    type Response = LtSlimAPIJSON<LtCorePutOrganizationsKeysSignatureRes>;
    type Body<'a> = LtSlimAPIJSON<&'a LtCorePutOrganizationsKeysSignatureBody>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(&self.body)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/organizations/keys/signature"))
    }
}

impl AuthReq for LtCorePutOrganizationsKeysSignatureReq {}
