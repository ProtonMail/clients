use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetOrganizationsKeysSignatureReq;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetOrganizationsKeysSignatureRes {
    /// Organization public key (PGP).
    pub public_key: String,

    /// Signature of the SHA256 fingerprint of the organization key.
    pub fingerprint_signature: String,

    /// Admin email (or display string) for the signing user — **not** an encrypted
    /// [`crate::auth::LtAuthAddressId`] (unlike `SignatureAddress` on
    /// [`crate::core::get_organizations_keys::LtCoreGetOrganizationsKeysRes`]).
    pub fingerprint_signature_address: String,
}

impl LtContract for LtCoreGetOrganizationsKeysSignatureReq {
    type Response = LtSlimAPIJSON<LtCoreGetOrganizationsKeysSignatureRes>;
    type Body<'a> = LtSlimAPIJSON<()>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/organizations/keys/signature"))
    }
}

impl AuthReq for LtCoreGetOrganizationsKeysSignatureReq {}
