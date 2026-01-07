use std::borrow::Cow;

use crate::{AuthReq, LatticeContract, LatticeError, auth::LtAuthAddressId, core::LtCoreAddress};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetAddressRes {
    pub address: LtCoreAddress,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetAddressReq {
    pub id: LtAuthAddressId,
}

impl LatticeContract for LtCoreGetAddressReq {
    type Response = LtCoreGetAddressRes;
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/core/v4/addresses/{}", self.id.0)))
    }
}

impl AuthReq for LtCoreGetAddressReq {}
