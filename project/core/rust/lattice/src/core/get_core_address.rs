use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, auth::LtAuthAddressId,
    core::LtCoreAddress,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetAddressRes {
    pub address: LtCoreAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetAddressReq {
    pub id: LtAuthAddressId,
}

impl LtContract for LtCoreGetAddressReq {
    type Response = LtSlimAPIJSON<LtCoreGetAddressRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/core/v4/addresses/{}", self.id.0)))
    }
}

impl AuthReq for LtCoreGetAddressReq {}
