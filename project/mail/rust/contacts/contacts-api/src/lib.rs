//! Shared API contact types.

mod api;
mod requests;
mod responses;

pub use api::ContactApi;
pub use requests::{GetContactsEmailsOptions, GetContactsOptions, PutDeleteContacts};
pub use responses::{
    GetContactResponse, GetContactsEmailsResponse, GetContactsResponse, PutDeleteContactResponse,
    PutDeleteContactsResponse,
};

use mail_api_event_types::Action;
use mail_api_labels::LabelId;
use mail_proton_ids::PrivateEmail;
use proton_crypto_account::contacts::ContactCardType;
use serde::Deserialize;
#[cfg(feature = "mocks")]
use serde::Serialize;
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{BoolFromInt, serde_as};

mail_proton_ids::declare_proton_id! {
    pub ContactId
}

mail_proton_ids::declare_proton_id! {
    pub ContactEmailId
}

mail_proton_ids::declare_proton_id! {
    pub ContactUID
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Eq, Hash, PartialEq, Serialize_repr)]
#[repr(u8)]
pub enum ContactSendingPreferences {
    Custom = 0,
    Default = 1,
}

/// Represents partial contact information returned by the API.
///
/// The partial contact information does not contain the contact emails and the
/// v-cards.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactBasic {
    #[serde(rename = "ID")]
    pub id: ContactId,
    pub create_time: u64,
    #[serde(rename = "LabelIDs")]
    pub label_ids: Vec<LabelId>,
    pub modify_time: u64,
    pub name: String,
    pub size: u64,
    #[serde(rename = "UID")]
    pub uid: ContactUID,
}

/// Represents a contact card returned by the API.
///
/// Contact cards contain information encoded as a v-card. Cards can be
/// encrypted or signed with the user keys.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactCard {
    #[serde(rename = "Type")]
    pub card_type: ContactCardType,
    pub data: String,
    pub signature: Option<String>,
}

/// Models the contact email addresses for a contact returned by the API.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactEmail {
    #[serde(rename = "ID")]
    pub id: ContactEmailId,
    #[serde(rename = "ContactID")]
    pub contact_id: ContactId,
    pub canonical_email: PrivateEmail,
    #[serde(rename = "Type")]
    pub contact_type: Vec<String>,
    pub defaults: ContactSendingPreferences,
    pub email: PrivateEmail,
    #[serde_as(as = "BoolFromInt")]
    pub is_proton: bool,
    #[serde(rename = "LabelIDs")]
    pub label_ids: Vec<LabelId>,
    pub last_used_time: u64,
    pub name: String,
    pub order: u32,
}

/// Data for an event related to a [`ContactEmail`] record.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactEmailEvent {
    #[serde(rename = "ID")]
    pub id: ContactEmailId,
    pub action: Action,
    pub contact_email: Option<ContactEmail>,
}

/// Data for an event related to a [`ContactBasic`] record.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactEvent {
    #[serde(rename = "ID")]
    pub id: ContactId,
    pub action: Action,
    pub contact: Option<ContactFull>,
}

/// A complete contact returned by the API.
///
/// Compared to the [`ContactBasic`], it additionally includes all associated
/// contact emails ([`ContactEmail`]) and cards ([`ContactCard`]).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactFull {
    #[serde(rename = "ID")]
    pub id: ContactId,
    pub cards: Vec<ContactCard>,
    pub contact_emails: Vec<ContactEmail>,
    pub create_time: u64,
    #[serde(rename = "LabelIDs")]
    pub label_ids: Vec<LabelId>,
    pub modify_time: u64,
    pub name: String,
    pub size: u64,
    #[serde(rename = "UID")]
    pub uid: ContactUID,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactRootEventV6 {
    pub contacts: Option<Vec<ContactEventV6>>,
    pub labels: Option<Vec<ContactLabelEventV6>>,
    pub refresh: bool,
    /// Whether we need to request more events after this.
    #[serde(rename = "More")]
    pub has_more: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactEventV6 {
    #[serde(rename = "ID")]
    pub id: ContactId,
    pub action: Action,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactLabelEventV6 {
    #[serde(rename = "ID")]
    pub id: LabelId,
    pub action: Action,
}
