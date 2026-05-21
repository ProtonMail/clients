use std::borrow::Cow;
use std::collections::HashMap;

use crate::{
    LatticeError, LtContract, LtRequestQueryParams, LtSlimAPIJSON, Sensitive, UnauthReq,
    core::user::LtCoreParseDomain,
};

pub struct LtCoreGetUsersAvailableReq {
    /// The username to check for availability.
    pub name: String,
    /// Indicates whether the username should be parsed as a full email address.
    pub parse_domain: LtCoreParseDomain,

    /// The payment info token, if any.
    pub payment_info_token: Option<String>,
}

pub struct LtCoreGetUsersAvailableQueryParams<'a> {
    pub name: &'a str,
    pub parse_domain: u8,
}

impl LtRequestQueryParams for LtCoreGetUsersAvailableQueryParams<'_> {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        Ok(HashMap::from([
            ("Name".into(), Sensitive::new(self.name.to_owned())),
            (
                "ParseDomain".into(),
                Sensitive::new(self.parse_domain.to_string()),
            ),
        ]))
    }
}

impl LtContract for LtCoreGetUsersAvailableReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtCoreGetUsersAvailableQueryParams<'q>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users/available"))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        let parse_domain: u8 = self.parse_domain.into();
        Some(LtCoreGetUsersAvailableQueryParams {
            name: self.name.as_str(),
            parse_domain,
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

impl UnauthReq for LtCoreGetUsersAvailableReq {}
