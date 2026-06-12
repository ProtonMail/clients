use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, UnauthReq};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtAuthGetSessionsForksReq;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthGetSessionsForksRes {
    /// Random 20-char selector for polling
    pub selector: String,

    /// Random 8-char user code for display
    pub user_code: String,
}

impl LtContract for LtAuthGetSessionsForksReq {
    type Response = LtSlimAPIJSON<LtAuthGetSessionsForksRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/sessions/forks"))
    }
}

impl UnauthReq for LtAuthGetSessionsForksReq {}
