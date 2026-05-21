use std::borrow::Cow;
use std::collections::HashMap;

use crate::{LatticeError, LtContract, LtRequestQueryParams, LtSlimAPIJSON, Sensitive, UnauthReq};

pub struct LtCoreGetUsersAvailableExternalReq {
    /// The username to check for availability.
    pub name: String,

    /// The payment info token, if any.
    pub payment_info_token: Option<String>,
}

pub struct LtCoreGetUsersAvailableExternalQueryParams<'a> {
    pub name: &'a str,
}

impl LtRequestQueryParams for LtCoreGetUsersAvailableExternalQueryParams<'_> {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        Ok(HashMap::from([(
            Cow::from("Name"),
            Sensitive::new(self.name.to_owned()),
        )]))
    }
}

impl LtContract for LtCoreGetUsersAvailableExternalReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtCoreGetUsersAvailableExternalQueryParams<'q>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users/availableExternal"))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        Some(LtCoreGetUsersAvailableExternalQueryParams {
            name: self.name.as_str(),
        })
    }

    fn headers(&self) -> Result<HashMap<String, Sensitive<String>>, LatticeError> {
        Ok(self
            .payment_info_token
            .as_ref()
            .map(|token| {
                HashMap::from([(
                    "X-PM-Payment-Info-Token".to_string(),
                    Sensitive::new(token.clone()),
                )])
            })
            .unwrap_or_default())
    }
}

impl UnauthReq for LtCoreGetUsersAvailableExternalReq {}
