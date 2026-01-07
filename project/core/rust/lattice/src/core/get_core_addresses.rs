use std::borrow::Cow;

use crate::{AuthReq, LatticeContract, LatticeError, core::LtCoreAddress};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetAddressesRes {
    pub addresses: Vec<LtCoreAddress>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetAddressesReq;

impl LatticeContract for LtCoreGetAddressesReq {
    type Response = LtCoreGetAddressesRes;
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/addresses"))
    }
}

impl AuthReq for LtCoreGetAddressesReq {}
