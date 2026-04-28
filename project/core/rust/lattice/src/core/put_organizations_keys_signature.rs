use std::borrow::Cow;

use crate::auth::LtAuthAddressId;
use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON, Method, Sensitive};

/// `PUT /core/v4/organizations/keys/signature` — publish org-identity (fingerprint) signature.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCorePutOrganizationsKeysSignatureReq {
    pub body: LtCorePutOrganizationsKeysSignatureBody,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePutOrganizationsKeysSignatureBody {
    /// Armored detached PGP signature over the org key SHA-256 fingerprint.
    pub signature: Sensitive<String>,

    /// Encrypted address id of the signing key.
    #[cfg_attr(feature = "serde", serde(rename = "AddressID"))]
    pub address_id: LtAuthAddressId,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LtCorePutOrganizationsKeysSignatureRes {}

impl LtContract for LtCorePutOrganizationsKeysSignatureReq {
    type Response = LtSlimAPIJSON<LtCorePutOrganizationsKeysSignatureRes>;
    type Body<'a> = LtSlimAPIJSON<&'a LtCorePutOrganizationsKeysSignatureBody>;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(&self.body)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/organizations/keys/signature"))
    }
}

impl AuthReq for LtCorePutOrganizationsKeysSignatureReq {}
