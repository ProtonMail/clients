use std::collections::{BTreeMap, HashMap};

use contact_avatar::AvatarInformation;
use contact_database::{Contact, ContactEmail, ContactGroup, LocalContactGroupId, LocalContactId};
use mail_proton_ids::PrivateEmail;
use unicode_segmentation::UnicodeSegmentation;

const DEFAULT_GROUP: &str = "#";

/// This is the main data structure that is used to represent the group of contacts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupedContacts {
    /// The field represent first grapheme of the name of the contact
    pub grouped_by: String,

    /// The field represent the list of contacts or groups for the given grapheme
    pub items: Vec<ContactItemType>,
}

impl GroupedContacts {
    /// Builds the alphabetically bucketed list of contacts and contact groups.
    ///
    /// `contacts` carries each contact paired with the emails attached to it
    /// — the caller is responsible for that join because
    /// [`contact_database::Contact`] does not embed emails.
    ///
    /// `contact_groups` carries every contact group; groups without a
    /// `remote_id` are dropped (they have not yet been synced to the server
    /// and cannot be displayed).
    #[must_use]
    pub fn from_contacts_and_groups(
        mut contacts: Vec<(Contact, Vec<ContactEmail>)>,
        contact_groups: Vec<ContactGroup>,
    ) -> Vec<Self> {
        let mut btmap: BTreeMap<String, Vec<ContactItemType>> = BTreeMap::new();

        let mut contact_group_items: HashMap<
            LocalContactGroupId,
            (ContactGroupItem, Vec<ContactEmail>),
        > = contact_groups
            .into_iter()
            .filter(|group| group.remote_id.is_some())
            .map(|group| {
                (
                    group.local_id,
                    (
                        ContactGroupItem {
                            local_id: group.local_id,
                            avatar_information: AvatarInformation::from(&group.name),
                            name: group.name,
                            contacts: vec![],
                        },
                        vec![],
                    ),
                )
            })
            .collect();

        contacts.sort_by_key(|(contact, _)| contact.name.unicode_words().collect::<String>());

        for (_, emails) in &contacts {
            for email in emails {
                for id in &email.label_ids {
                    if let Some((_, bucket)) = contact_group_items.get_mut(id) {
                        bucket.push(email.clone());
                    }
                }
            }
        }

        let groups = contact_group_items
            .into_values()
            .map(|(mut group, mut emails)| {
                emails.sort_unstable_by_key(|email| (email.display_order, email.local_id));
                group.contacts = emails.into_iter().map(ContactEmailItem::from).collect();
                ContactItemType::from(group)
            });

        contacts
            .into_iter()
            .map(|(contact, emails)| ContactItem::build(contact, emails))
            .map(ContactItemType::from)
            .chain(groups)
            .for_each(|item| {
                let key = item.key();
                let key = if key.is_empty() || key == "?" {
                    DEFAULT_GROUP
                } else {
                    key
                };

                btmap.entry(key.to_owned()).or_default().push(item);
            });

        btmap
            .into_iter()
            .map(|(grouped_by, items)| GroupedContacts { grouped_by, items })
            .collect()
    }
}

/// List of contacts is composed of contacts and groups.
#[derive(Clone, Debug, PartialEq)]
pub enum ContactItemType {
    Contact(ContactItem),
    Group(ContactGroupItem),
}

impl ContactItemType {
    /// Represents the first grapheme in the contact list, used to sort the contacts alphabetically
    fn key(&self) -> &str {
        let avatar_information = match self {
            ContactItemType::Contact(contact_item) => &contact_item.avatar_information,
            ContactItemType::Group(contact_group_item) => &contact_group_item.avatar_information,
        };

        avatar_information.text.as_str()
    }
}

impl From<ContactItem> for ContactItemType {
    fn from(value: ContactItem) -> Self {
        Self::Contact(value)
    }
}

impl From<ContactGroupItem> for ContactItemType {
    fn from(value: ContactGroupItem) -> Self {
        Self::Group(value)
    }
}

/// This is the main data structure that is used to represent the contact.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactItem {
    pub local_id: LocalContactId,
    pub name: String,
    pub avatar_information: AvatarInformation,
    pub emails: Vec<ContactEmailItem>,
}

impl ContactItem {
    fn build(contact: Contact, emails: Vec<ContactEmail>) -> Self {
        let avatar_information = AvatarInformation::from(&contact.name)
            .or_else(
                emails
                    .first()
                    .map(|email| email.email.as_clear_text_str())
                    .unwrap_or_default(),
            )
            .or_else_unchecked("?");

        Self {
            local_id: contact.local_id,
            avatar_information,
            emails: emails.into_iter().map(ContactEmailItem::from).collect(),
            name: contact.name,
        }
    }
}

/// This is the main data structure that is used to represent the contact group.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactGroupItem {
    pub local_id: LocalContactGroupId,
    pub name: String,
    pub avatar_information: AvatarInformation,
    pub contacts: Vec<ContactEmailItem>,
}

/// This is the main data structure that is used to represent the contact email.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactEmailItem {
    pub local_contact_id: LocalContactId,
    pub email: PrivateEmail,
    /// The field represents if the email is a proton email like foo@pm.me
    pub is_proton: bool,
    pub last_used_time: u64,
    pub name: String,
    pub avatar_information: AvatarInformation,
}

impl From<ContactEmail> for ContactEmailItem {
    fn from(value: ContactEmail) -> Self {
        let name = if value.name.is_empty() {
            value.email.clone().into_clear_text_string()
        } else {
            value.name
        };

        Self {
            local_contact_id: value.local_contact_id,
            email: value.email,
            is_proton: value.is_proton,
            last_used_time: value.last_used_time,
            avatar_information: AvatarInformation::from(&name),
            name,
        }
    }
}

#[cfg(test)]
mod tests;
