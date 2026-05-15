use std::{borrow::Cow, collections::HashMap, iter::once};

use proton_crypto_account::keys::APIPublicAddressKeys;

use crate::{AuthReq, LatticeError, LtContract, LtRequestQueryParams, LtSlimAPIJSON, Sensitive};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetKeysAllReq {
    pub email: String,
}

pub struct LtCoreGetKeysAllQueryParams<'a> {
    pub email: &'a str,
}

impl LtRequestQueryParams for LtCoreGetKeysAllQueryParams<'_> {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        Ok(once(("Email".into(), Sensitive::new(self.email.to_owned()))).collect())
    }
}

impl LtContract for LtCoreGetKeysAllReq {
    type Response = LtSlimAPIJSON<APIPublicAddressKeys>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtCoreGetKeysAllQueryParams<'q>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/keys/all"))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        Some(LtCoreGetKeysAllQueryParams {
            email: self.email.as_str(),
        })
    }
}

impl AuthReq for LtCoreGetKeysAllReq {}
