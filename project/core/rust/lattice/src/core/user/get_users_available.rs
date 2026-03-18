use std::borrow::Cow;
use std::collections::HashMap;

use crate::{LatticeError, LtContract, LtSlimAPIJSON, UnauthReq, core::user::LtCoreParseDomain};

pub struct LtCoreGetUsersAvailableReq {
    /// The username to check for availability.
    pub name: String,
    /// Indicates whether the username should be parsed as a full email address.
    pub parse_domain: LtCoreParseDomain,

    /// The payment info token, if any.
    pub payment_info_token: Option<String>,
}

impl LtContract for LtCoreGetUsersAvailableReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<()>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users/available"))
    }

    fn query(&self) -> Result<Option<HashMap<String, String>>, LatticeError> {
        let parse_domain: u8 = self.parse_domain.into();
        Ok(Some(HashMap::from([
            ("Name".to_string(), self.name.clone()),
            ("ParseDomain".to_string(), parse_domain.to_string()),
        ])))
    }

    fn headers(&self) -> Result<HashMap<String, String>, LatticeError> {
        Ok(self
            .payment_info_token
            .as_ref()
            .map(|token| HashMap::from([("X-PM-Payment-Info-Token".to_string(), token.clone())]))
            .unwrap_or_default())
    }
}

impl UnauthReq for LtCoreGetUsersAvailableReq {}
