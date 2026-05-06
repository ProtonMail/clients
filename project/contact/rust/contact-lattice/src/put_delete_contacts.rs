use std::borrow::Cow;

use crate::{CONTACTS_V4, ContactId};
use lattice::{LtApiResponseErrorInfo, LtContract, LtNoQueryParams, LtSlimAPIJSON};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PutDeleteContacts {
    /// The list of contact IDs to delete.
    #[serde(rename = "IDs")]
    pub ids: Vec<ContactId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct PutDeleteContactsResponse {
    /// List of responses.
    pub responses: Vec<PutDeleteContactResponse>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct PutDeleteContactResponse {
    pub response: LtApiResponseErrorInfo<u32, serde_json::Value>,
    /// Remote ID of the contact.
    #[serde(rename = "ID")]
    pub id: ContactId,
}

impl LtContract for PutDeleteContacts {
    type Response = LtSlimAPIJSON<PutDeleteContactsResponse>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<std::borrow::Cow<'a, str>, lattice::LatticeError> {
        Ok(Cow::Owned(format!("{CONTACTS_V4}/delete")))
    }

    fn method<'a>(&'a self) -> Result<lattice::Method<Self::Body<'a>>, lattice::LatticeError> {
        Ok(lattice::Method::Put(LtSlimAPIJSON(self)))
    }
}

#[cfg(feature = "mocks")]
impl PutDeleteContacts {
    pub fn mock() -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("PUT")).and(path(format!("api{CONTACTS_V4}/delete")))
    }
}
