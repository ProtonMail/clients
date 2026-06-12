use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{LtContract, LtNoQueryParams, LtSlimAPIJSON, Sensitive, UnauthReq};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtAuthGetModulusReq;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthGetModulusRes {
    #[serde(rename = "ModulusID")]
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
