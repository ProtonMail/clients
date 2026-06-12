use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{LtContract, LtNoQueryParams, LtSlimAPIJSON, UnauthReq, auth::LtAuthApiSession};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtAuthGetSessionsForksByIdReq {
    pub selector: String,
}

impl UnauthReq for LtAuthGetSessionsForksByIdReq {}

impl LtContract for LtAuthGetSessionsForksByIdReq {
    type Response = LtSlimAPIJSON<LtAuthGetSessionsForksByIdRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, crate::LatticeError> {
        Ok(Cow::Owned(format!(
            "/auth/v4/sessions/forks/{}",
            self.selector
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthGetSessionsForksByIdRes {
    /// Base64-encoded encrypted payload (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// Authentication tokens
    #[serde(flatten)]
    pub session: LtAuthApiSession,
}
