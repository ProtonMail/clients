mod delete_contact_group;
mod get_contact;
mod get_contact_events;
mod get_contact_group;
mod get_contacts_emails;
mod post_contact_group;
mod put_contact_group;
mod put_delete_contacts;

pub use delete_contact_group::*;
pub use get_contact::*;
pub use get_contact_events::*;
pub use get_contact_group::*;
pub use get_contacts_emails::*;
pub use post_contact_group::*;
use proton_crypto_account::contacts::ContactCardType;
pub use put_contact_group::*;
pub use put_delete_contacts::*;

use mail_api_event_types::Action;
use mail_proton_ids::PrivateEmail;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{BoolFromInt, DefaultOnNull, serde_as};

const CONTACTS_V4: &str = "/contacts/v4/contacts";
const CONTACTS_V6: &str = "/contacts/v6";
const CORE_V4: &str = "/core/v4";
const CONTACT_GROUP_LABEL_TYPE: u32 = 2;

mail_proton_ids::declare_proton_id! {
    pub ContactId
}

mail_proton_ids::declare_proton_id! {
    pub ContactEmailId
}

mail_proton_ids::declare_proton_id! {
    pub ContactUID
}

mail_proton_ids::declare_proton_id! {
    pub ContactGroupId
}

#[cfg(feature = "mail-utils")]
impl From<ContactGroupId> for mail_api_labels::LabelId {
    fn from(value: ContactGroupId) -> Self {
        Self::from(value.into_inner())
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize_repr,
    Eq,
    Hash,
    PartialEq,
    Serialize_repr
)]
#[repr(u8)]
pub enum ContactSendingPreferences {
    Custom = 0,
    Default = 1,
}

/// Represents partial contact information returned by the API.
///
/// The partial contact information does not contain the contact emails and the
/// v-cards.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ContactBasic {
    #[serde(rename = "ID")]
    pub id: ContactId,
    pub create_time: u64,
    #[serde(rename = "LabelIDs")]
    pub label_ids: Vec<ContactGroupId>,
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
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ContactCard {
    #[serde(rename = "Type")]
    pub card_type: ContactCardType,
    pub data: String,
    pub signature: Option<String>,
}

/// Models the contact email addresses for a contact returned by the API.
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
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
    pub label_ids: Vec<ContactGroupId>,
    pub last_used_time: u64,
    pub name: String,
    pub order: u32,
}

/// Data for an event related to a [`ContactEmail`] record.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(serde::Serialize))]
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
#[cfg_attr(feature = "mocks", derive(serde::Serialize))]
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
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ContactFull {
    #[serde(rename = "ID")]
    pub id: ContactId,
    pub cards: Vec<ContactCard>,
    pub contact_emails: Vec<ContactEmail>,
    pub create_time: u64,
    #[serde(rename = "LabelIDs")]
    pub label_ids: Vec<ContactGroupId>,
    pub modify_time: u64,
    pub name: String,
    pub size: u64,
    #[serde(rename = "UID")]
    pub uid: ContactUID,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(serde::Serialize))]
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
#[cfg_attr(feature = "mocks", derive(serde::Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactEventV6 {
    #[serde(rename = "ID")]
    pub id: ContactId,
    pub action: Action,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(serde::Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct ContactLabelEventV6 {
    #[serde(rename = "ID")]
    pub id: ContactGroupId,
    pub action: Action,
}

// Even though they are techincally sharing the same types as labels, we
// only need this subset of data to work.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(serde::Serialize))]
#[serde(rename_all = "PascalCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct ContactGroup {
    #[serde(rename = "ID")]
    pub id: ContactGroupId,

    pub color: String,

    #[serde_as(as = "DefaultOnNull<BoolFromInt>")]
    pub display: bool,

    pub name: String,

    #[serde(default)]
    pub order: u32,

    #[serde_as(as = "DefaultOnNull<BoolFromInt>")]
    pub sticky: bool,
}

#[cfg(feature = "mail-utils")]
impl From<ContactGroup> for mail_api_labels::Label {
    fn from(value: ContactGroup) -> Self {
        let path = value.name.clone();
        Self {
            id: value.id.into(),
            color: value.color,
            display: value.display,
            label_type: mail_api_labels::LabelType::ContactGroup,
            name: value.name,
            order: value.order,
            path: Some(path),
            sticky: value.sticky,
            ..Self::test_default()
        }
    }
}
