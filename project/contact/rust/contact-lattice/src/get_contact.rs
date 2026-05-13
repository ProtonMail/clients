use std::borrow::Cow;

use crate::{CONTACTS_V4, ContactBasic, ContactFull, ContactGroupId, ContactId};
use lattice::{LtContract, LtNoQueryParams, LtSerdeQueryParams, LtSlimAPIJSON};
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

pub struct GetContactRequest {
    pub id: ContactId,
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(serde::Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct GetContactResponse {
    pub contact: ContactFull,
}

#[cfg(feature = "mail-utils")]
impl mail_api_utils::PaginateResponse<ContactBasic> for GetContactsResponse {
    fn total(&self) -> u64 {
        self.total
    }

    fn items(self) -> Vec<ContactBasic> {
        self.contacts
    }
}

impl LtContract for GetContactRequest {
    type Response = LtSlimAPIJSON<GetContactResponse>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<std::borrow::Cow<'a, str>, lattice::LatticeError> {
        Ok(Cow::Owned(format!("{CONTACTS_V4}/{}", self.id.0)))
    }
}

#[cfg(feature = "mocks")]
impl GetContactRequest {
    pub fn mock(id: ContactId) -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("GET")).and(path(format!("api{CONTACTS_V4}/{id}")))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct GetContactsResponse {
    pub contacts: Vec<ContactBasic>,

    pub total: u64,
}

#[derive(Clone, Debug, Serialize, SmartDefault)]
#[serde(rename_all = "PascalCase")]
pub struct GetContactsOptions {
    /// Label ID to filter on.
    #[serde(rename = "LabelID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_id: Option<ContactGroupId>,

    /// Page index, i.e. the page in the resultset.
    pub page: u64,

    /// Number of records per page.
    #[default(MAX_PAGE_ELEMENT_COUNT)]
    pub page_size: u64,
}

#[cfg(feature = "mail-utils")]
impl mail_api_utils::PaginateOptions for GetContactsOptions {
    fn from_zero(page_size: u64) -> Self {
        Self {
            page: 0,
            page_size,
            ..Default::default()
        }
    }

    fn with_page(self, page: u64) -> Self {
        Self { page, ..self }
    }

    fn size(&self) -> u64 {
        self.page_size
    }
}

pub struct GetContactsRequest {
    pub options: GetContactsOptions,
}

#[cfg(feature = "mocks")]
impl GetContactsRequest {
    pub fn mock() -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("GET")).and(path(format!("api{CONTACTS_V4}")))
    }
}

impl LtContract for GetContactsRequest {
    type Response = LtSlimAPIJSON<GetContactsResponse>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtSerdeQueryParams<&'q GetContactsOptions>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, lattice::LatticeError> {
        Ok(Cow::Borrowed(CONTACTS_V4))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        Some(LtSerdeQueryParams::new(&self.options))
    }
}

const MAX_PAGE_ELEMENT_COUNT: u64 = 200;
