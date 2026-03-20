//! Contact API request structures.

use crate::ContactId;
use mail_api_labels::LabelId;
use mail_api_utils::PaginateOptions;
use mail_proton_ids::PrivateEmail;
use serde::Serialize;
use smart_default::SmartDefault;

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
    pub label_id: Option<LabelId>,

    /// Page index, i.e. the page in the resultset.
    pub page: u64,

    /// Number of records per page.
    #[default(MAX_PAGE_ELEMENT_COUNT)]
    pub page_size: u64,
}

impl PaginateOptions for GetContactsEmailsOptions {
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

/// Parameters for getting contacts.
#[derive(Clone, Debug, Serialize, SmartDefault)]
#[serde(rename_all = "PascalCase")]
pub struct GetContactsOptions {
    /// Label ID to filter on.
    #[serde(rename = "LabelID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_id: Option<LabelId>,

    /// Page index, i.e. the page in the resultset.
    pub page: u64,

    /// Number of records per page.
    #[default(MAX_PAGE_ELEMENT_COUNT)]
    pub page_size: u64,
}

impl PaginateOptions for GetContactsOptions {
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PutDeleteContacts {
    /// The list of contact IDs to delete.
    #[serde(rename = "IDs")]
    pub ids: Vec<ContactId>,
}
