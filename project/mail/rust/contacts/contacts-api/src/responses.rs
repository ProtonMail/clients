//! Contact API response structures.

use crate::{ContactBasic, ContactEmail, ContactFull, ContactId};
use mail_api_shared::ApiErrorInfo;
use mail_api_utils::PaginateResponse;
use serde::Deserialize;
#[cfg(feature = "mocks")]
use serde::Serialize;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct GetContactResponse {
    pub contact: ContactFull,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct GetContactsEmailsResponse {
    pub contact_emails: Vec<ContactEmail>,
    pub total: u64,
}

impl PaginateResponse<ContactEmail> for GetContactsEmailsResponse {
    fn total(&self) -> u64 {
        self.total
    }

    fn items(self) -> Vec<ContactEmail> {
        self.contact_emails
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct GetContactsResponse {
    pub contacts: Vec<ContactBasic>,

    pub total: u64,
}

impl PaginateResponse<ContactBasic> for GetContactsResponse {
    fn total(&self) -> u64 {
        self.total
    }

    fn items(self) -> Vec<ContactBasic> {
        self.contacts
    }
}

/// The response containing information about deletion of the contacts.
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
    /// Remote ID of the contact.
    #[serde(rename = "ID")]
    pub id: ContactId,
    /// Response data.
    pub response: ApiErrorInfo,
}
