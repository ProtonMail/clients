use std::borrow::Cow;

use crate::{LtContract, LtNoQueryParams, LtSlimAPIJSON, Sensitive, UnauthReq};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthGetModulusReq;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthGetModulusRes {
    #[cfg_attr(feature = "serde", serde(rename = "ModulusID"))]
    pub modulus_id: String,
    pub modulus: Sensitive<String>,
}

impl LtContract for LtAuthGetModulusReq {
    type Response = LtSlimAPIJSON<LtAuthGetModulusRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, crate::LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/modulus"))
    }
}

impl UnauthReq for LtAuthGetModulusReq {}
