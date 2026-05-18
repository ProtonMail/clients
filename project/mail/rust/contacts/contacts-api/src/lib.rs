//! Shared API contact types.

mod api;
#[cfg(feature = "mocks")]
pub mod mocks;

pub use api::ContactApi;
pub use contact_lattice::{
    ContactBasic, ContactCard, ContactEmail, ContactEvent, ContactEventV6, ContactFull,
    ContactGroup, ContactGroupId, ContactLabelEventV6, ContactRootEventV6,
    ContactSendingPreferences, GetContactEventLatestRequest, GetContactResponse,
    GetContactsEmailsOptions, GetContactsEmailsResponse, GetContactsOptions, GetContactsResponse,
    PutDeleteContactResponse, PutDeleteContactsRequest, PutDeleteContactsResponse,
};

pub use contact_lattice::{ContactEmailId, ContactId, ContactUID};
use mail_api_event_types::Action;
use serde_with::serde_as;

/// Data for an event related to a [`ContactEmail`] record.
#[serde_as]
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(serde::Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactEmailEvent {
    #[serde(rename = "ID")]
    pub id: ContactEmailId,
    pub action: Action,
    pub contact_email: Option<ContactEmail>,
}
