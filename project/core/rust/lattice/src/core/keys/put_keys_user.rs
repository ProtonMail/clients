use std::borrow::Cow;
use std::collections::HashMap;

use crate::{
    AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive,
    auth::LtAuthUserKeyId, core::LtCoreSignedKeyList,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePutKeysUserBody {
    pub private_key: Sensitive<String>,

    pub address_key_fingerprints: Vec<String>,

    pub signed_key_lists: HashMap<String, LtCoreSignedKeyList>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub struct LtCorePutKeysUserReq {
    pub user_key_id: LtAuthUserKeyId,
    pub body: LtCorePutKeysUserBody,
}

impl LtContract for LtCorePutKeysUserReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<&'a LtCorePutKeysUserBody>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Put(LtSlimAPIJSON(&self.body)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/keys/user/{}",
            self.user_key_id
        )))
    }
}

impl AuthReq for LtCorePutKeysUserReq {}
