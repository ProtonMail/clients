use std::{borrow::Cow, collections::HashMap, iter::once};

use proton_crypto_account::keys::APIPublicAddressKeys;

use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetKeysAllReq {
    pub email: String,
}

impl LtContract for LtCoreGetKeysAllReq {
    type Response = LtSlimAPIJSON<APIPublicAddressKeys>;
    type Body<'a> = LtSlimAPIJSON<()>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/keys/all"))
    }

    fn query(&self) -> Result<Option<HashMap<String, String>>, LatticeError> {
        Ok(Some(
            once((String::from("Email"), self.email.clone())).collect(),
        ))
    }
}

impl AuthReq for LtCoreGetKeysAllReq {}
