use std::borrow::Cow;

use lattice::{LtContract, LtSerdeQueryParams, LtSlimAPIJSON};
use mail_proton_ids::PrivateEmail;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use crate::{CONTACTS_V4, ContactEmail, ContactGroupId};

const MAX_PAGE_ELEMENT_COUNT: u64 = 200;

/// Parameters for getting emails for contacts.
#[derive(Clone, Debug, Serialize, SmartDefault)]
#[serde(rename_all = "PascalCase")]
pub struct GetContactsEmailsOptions {
    /// Email address to filter on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<PrivateEmail>,

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
impl mail_api_utils::PaginateOptions for GetContactsEmailsOptions {
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct GetContactsEmailsResponse {
    pub contact_emails: Vec<ContactEmail>,
    pub total: u64,
}

#[cfg(feature = "mail-utils")]
impl mail_api_utils::PaginateResponse<ContactEmail> for GetContactsEmailsResponse {
    fn total(&self) -> u64 {
        self.total
    }

    fn items(self) -> Vec<ContactEmail> {
        self.contact_emails
    }
}

pub struct GetContactsEmails {
    pub options: GetContactsEmailsOptions,
}

impl LtContract for GetContactsEmails {
    type Response = LtSlimAPIJSON<GetContactsEmailsResponse>;
    type Body<'b> = LtSlimAPIJSON<()>;

    type Query<'q> = LtSerdeQueryParams<&'q GetContactsEmailsOptions>;

    fn path<'a>(&'a self) -> Result<std::borrow::Cow<'a, str>, lattice::LatticeError> {
        Ok(Cow::Owned(format!("{CONTACTS_V4}/emails")))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        Some(LtSerdeQueryParams::new(&self.options))
    }
}

#[cfg(feature = "mocks")]
impl GetContactsEmails {
    pub fn mock() -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("GET")).and(path(format!("api{CONTACTS_V4}/emails")))
    }
}
