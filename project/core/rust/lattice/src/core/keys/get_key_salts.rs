use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use proton_crypto_account::salts::Salts;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetKeysSaltsRes {
    pub key_salts: Salts,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetKeySaltsReq;

impl LtContract for LtCoreGetKeySaltsReq {
    type Response = LtSlimAPIJSON<LtCoreGetKeysSaltsRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/keys/salts"))
    }
}

impl AuthReq for LtCoreGetKeySaltsReq {}
