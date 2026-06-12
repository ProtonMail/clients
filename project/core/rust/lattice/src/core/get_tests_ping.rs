use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, UnauthReq};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCoreGetTestsPingReq;

impl LtContract for LtCoreGetTestsPingReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/tests/ping"))
    }
}

impl UnauthReq for LtCoreGetTestsPingReq {}
