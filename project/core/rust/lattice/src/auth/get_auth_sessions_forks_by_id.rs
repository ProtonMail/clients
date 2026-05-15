use std::borrow::Cow;

use crate::{LtContract, LtNoQueryParams, LtSlimAPIJSON, UnauthReq, auth::LtAuthApiSession};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthGetSessionsForksByIdRes {
    /// Base64-encoded encrypted payload (if any)
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub payload: Option<String>,
    /// Authentication tokens
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub session: LtAuthApiSession,
}
