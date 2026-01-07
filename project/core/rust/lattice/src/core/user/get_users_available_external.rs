use std::borrow::Cow;
use std::collections::HashMap;

use crate::{LatticeContract, LatticeError, UnauthReq};

pub struct LtCoreGetUsersAvailableExternalReq {
    /// The username to check for availability.
    pub name: String,

    /// The payment info token, if any.
    pub payment_info_token: Option<String>,
}

impl LatticeContract for LtCoreGetUsersAvailableExternalReq {
    type Response = ();
    type Body<'a> = ();

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users/availableExternal"))
    }

    fn query(&self) -> Result<Option<HashMap<String, String>>, LatticeError> {
        Ok(Some(HashMap::from([(
            "Name".to_string(),
            self.name.clone(),
        )])))
    }

    fn headers(&self) -> Result<HashMap<String, String>, LatticeError> {
        Ok(self
            .payment_info_token
            .as_ref()
            .map(|token| HashMap::from([("X-PM-Payment-Info-Token".to_string(), token.clone())]))
            .unwrap_or_default())
    }
}

impl UnauthReq for LtCoreGetUsersAvailableExternalReq {}
