use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::mem;

use itertools::Itertools;
use mail_avatar::AvatarInformation;
use mail_core_api::services::proton::{LabelId, PrivateEmail};
use mail_labels_common::{Label, LabelType};
use mail_shared_types::{MapVec as _, UnixTimestamp};
use mail_stash::orm::Model;
use unicode_segmentation::UnicodeSegmentation;

use crate::contact::Contact;
use crate::contact_email::ContactEmail;
use crate::local_ids::LocalContactId;
use mail_labels_common::LocalLabelId;

const DEFAULT_GROUP: &str = "#";

/// This is the main data structure that is used to represent the group of contacts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupedContacts {
    /// The field represent first grapheme of the name of the contact
    pub grouped_by: String,

    // The field represent the list of contacts or groups for the given grapheme
    pub items: Vec<ContactItemType>,
}

impl GroupedContacts {
    /// Builds grouped contacts based on flat contact list and contact groups
    ///
    /// # Contact groups
    ///
    /// Note, that the contact group is represented by [`Label`]. Currently, this function WON'T
    /// assert if the label has type `ContactGroup`.
    ///
    #[must_use]
    pub fn from_contacts_and_groups(
        mut contacts: Vec<Contact>,
        contact_groups: Vec<Label>,
    ) -> Vec<Self> {
        debug_assert!(
            contact_groups
                .iter()
                .all(|group| group.label_type == LabelType::ContactGroup)
        );

        let mut btmap: BTreeMap<String, Vec<ContactItemType>> = BTreeMap::new();

        let mut contact_group_items: HashMap<LabelId, (ContactGroupItem, Vec<ContactEmail>)> =
            contact_groups
                .into_iter()
                .filter(|group| group.remote_id.is_some())
                .filter(|group| group.label_type == LabelType::ContactGroup)
                .map(|group| {
                    let local_id = group.id();
                    (
                        group.remote_id.unwrap().clone(),
                        (
                            ContactGroupItem {
                                local_id,
                                name: group.name.clone(),
                                avatar_information: AvatarInformation::from(&group.name),
                                contacts: vec![],
                            },
                            vec![],
                        ),
                    )
                })
                .collect();

        contacts.sort_by_key(|c| c.name.unicode_words().collect::<String>());
        for contact in &contacts {
            for email in &contact.contact_emails {
                for id in email.label_ids.iter() {
                    if let Some((_, emails)) = contact_group_items.get_mut(id) {
                        emails.push(email.clone());
                    }
                }
            }
        }

        let groups = contact_group_items
            .into_values()
            .map(|(mut group, mut emails)| {
                emails.sort_unstable_by_key(|x| (x.display_order, x.id()));
                group.contacts = emails.map_vec();
                ContactItemType::from(group)
            });

        contacts
            .into_iter()
            .map_into::<ContactItem>()
            .map_into::<ContactItemType>()
            .chain(groups)
            .for_each(|contact| {
                let key = contact.key();
                let key = if key.is_empty() || key == "?" {
                    DEFAULT_GROUP
                } else {
                    key
                };

                btmap.entry(key.to_owned()).or_default().push(contact);
            });

        btmap
            .into_iter()
            .map(|(grouped_by, items)| GroupedContacts { grouped_by, items })
            .collect()
    }
}

/// List of contacts is composed of contacts and groups.
/// This enum is used to represent the either one.
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

impl From<Contact> for ContactItem {
    fn from(value: Contact) -> Self {
        Self {
            local_id: value.id(),
            avatar_information: AvatarInformation::from(&value.name)
                .or_else(
                    value
                        .contact_emails
                        .first()
                        .map(|email| email.email.as_clear_text_str())
                        .unwrap_or_default(),
                )
                .or_else_unchecked("?"),
            emails: value.contact_emails.map_vec(),
            name: value.name,
        }
    }
}

/// This is the main data structure that is used to represent the contact group.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactGroupItem {
    pub local_id: LocalLabelId,
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
    pub last_used_time: UnixTimestamp,
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
            // UNWRAP SAFETY: see ContactEmail::local_contact_id comment.
            local_contact_id: value.local_contact_id.expect("This should always be set"),
            email: value.email,
            is_proton: value.is_proton,
            last_used_time: value.last_used_time,
            avatar_information: AvatarInformation::from(&name),
            name,
        }
    }
}

/// Device contact feeded by the mobile/web application.
/// Used as an input for generating list of contact suggestions ([`ContactSuggestion`])
///
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeviceContact {
    /// The field represents unique key identifier used by the user to distinguish elements in the array
    pub key: String,

    /// The field represents the name of the contact
    pub name: String,

    /// List of email addresses assigned to the contact. That list has an arbitrary order given by the user
    pub emails: Vec<PrivateEmail>,
}

/// Collection of sorted contact suggestions
#[derive(Debug, PartialEq)]
pub struct ContactSuggestions {
    /// Sorted and deduplicated suggestions
    suggestions: Vec<ContactSuggestion>,
}

impl From<Vec<ContactSuggestion>> for ContactSuggestions {
    fn from(suggestions: Vec<ContactSuggestion>) -> Self {
        Self { suggestions }
    }
}

impl ContactSuggestions {
    /// Build contact suggestion list that is sorted and deduplicated
    ///
    #[must_use]
    pub fn from_contacts_and_device_contacts(
        contacts: Vec<Contact>,
        contact_groups: Vec<Label>,
        device_contacts: Vec<DeviceContact>,
    ) -> Self {
        debug_assert!(
            contact_groups
                .iter()
                .all(|group| group.label_type == LabelType::ContactGroup)
        );

        let label_ids = contacts
            .iter()
            .flat_map(|contact| {
                contact.label_ids.iter().cloned().chain(
                    contact
                        .contact_emails
                        .iter()
                        .flat_map(|email| email.label_ids.iter().cloned()),
                )
            })
            .collect::<HashSet<_>>();

        let mut contact_groups: HashMap<LabelId, ContactGroup> = contact_groups
            .into_iter()
            .filter(|group| group.remote_id.is_some())
            .filter(|group| group.label_type == LabelType::ContactGroup)
            .filter(|group| label_ids.contains(group.remote_id.as_ref().unwrap()))
            .map(|group| {
                let local_id = group.id();
                (
                    group.remote_id.unwrap(),
                    ContactGroup {
                        key: format!("group/{local_id}"),
                        name: group.name.clone(),
                        emails: vec![],
                    },
                )
            })
            .collect();

        let proton_suggestions: Vec<_> = contacts
            .into_iter()
            .filter(|contact| !contact.deleted)
            .flat_map(|contact| {
                contact
                    .contact_emails
                    .clone()
                    .into_iter()
                    .map(move |email| (contact.clone(), email))
            })
            .sorted_by_key(|(contact, email)| {
                (
                    !email.is_proton,
                    u64::MAX - email.last_used_time.as_u64(),
                    email.email.unicode_words().collect::<String>(),
                    contact.name.clone(),
                )
            })
            .map(|(contact, email)| {
                Self::aggregate_emails_to_groups(&mut contact_groups, contact, email)
            })
            .map(|(contact, email)| ContactSuggestion::new_contact(contact, email))
            .collect();

        let rest = contact_groups
            .into_values()
            .filter(|group| !group.emails.is_empty())
            .map(ContactSuggestion::new_group)
            .chain(
                device_contacts
                    .into_iter()
                    .map(ContactSuggestion::new_device_contact),
            )
            .sorted()
            .flat_map(|suggestion| match suggestion {
                FollowingSuggestion::ContactGroup(contact_suggestion) => vec![contact_suggestion],
                FollowingSuggestion::DeviceContact { suggestions, .. } => suggestions,
            });

        Self::concat_iters(proton_suggestions, rest)
    }

    pub fn concat(&mut self, other: Self) {
        let mut suggestions = vec![];
        mem::swap(&mut self.suggestions, &mut suggestions);
        *self = Self::concat_iters(suggestions, other.suggestions);
    }

    fn concat_iters(
        one: impl IntoIterator<Item = ContactSuggestion>,
        other: impl IntoIterator<Item = ContactSuggestion>,
    ) -> Self {
        Self {
            suggestions: one
                .into_iter()
                .chain(other)
                .unique_by(|suggestion| {
                    suggestion
                        .email()
                        .map(ToOwned::to_owned)
                        .map_or_else(|| suggestion.key.clone(), |email| email.to_lowercase())
                })
                .collect(),
        }
    }

    /// Return all contact suggestions
    ///
    #[must_use]
    pub fn all(&self) -> &[ContactSuggestion] {
        &self.suggestions
    }

    /// Return suggestions filtered by the query.
    ///
    #[must_use]
    pub fn filtered(&self, query: &str) -> Vec<ContactSuggestion> {
        let query = query.trim();
        let query = query.to_lowercase();

        if query.is_empty() {
            return Vec::new();
        }

        self.suggestions
            .iter()
            .filter(|suggestion| {
                suggestion.name.to_lowercase().contains(&query)
                    || suggestion
                        .email()
                        .is_some_and(|email| email.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }

    fn aggregate_emails_to_groups(
        contact_groups: &mut HashMap<LabelId, ContactGroup>,
        contact: Contact,
        mut email: ContactEmail,
    ) -> (Contact, ContactEmailItem) {
        let label_ids = mem::take(&mut email.label_ids);
        let email = ContactEmailItem::from(email);
        for label_id in label_ids.iter() {
            if let Some(group) = contact_groups.get_mut(label_id) {
                group.emails.push(email.clone());
            }
        }
        (contact, email)
    }
}

/// Used in the composer to suggest email addresses based on the user input (To:, CC: etc fields)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactSuggestion {
    pub key: String,
    pub name: String,
    pub avatar_information: AvatarInformation,
    pub kind: ContactSuggestionKind,
}

impl ContactSuggestion {
    #[must_use]
    pub fn email(&self) -> Option<&str> {
        match &self.kind {
            ContactSuggestionKind::ContactItem(contact_email_item) => {
                Some(contact_email_item.email.as_clear_text_str())
            }
            ContactSuggestionKind::DeviceContact(device_contact_suggestion) => {
                Some(device_contact_suggestion.email.as_clear_text_str())
            }
            ContactSuggestionKind::ContactGroup(_) => None,
        }
    }

    fn new_group(group: ContactGroup) -> FollowingSuggestion {
        FollowingSuggestion::ContactGroup(Self {
            key: group.key,
            avatar_information: AvatarInformation::from(&group.name),
            name: group.name,
            kind: ContactSuggestionKind::ContactGroup(group.emails),
        })
    }

    fn new_contact(contact: Contact, email: ContactEmailItem) -> Self {
        Self {
            key: format!("contact/{}", email.local_contact_id),
            avatar_information: AvatarInformation::from(&contact.name),
            name: contact.name,
            kind: ContactSuggestionKind::ContactItem(email),
        }
    }

    fn new_device_contact(contact: DeviceContact) -> FollowingSuggestion {
        FollowingSuggestion::DeviceContact {
            key: contact.key.clone(),
            name: contact.name.clone(),
            suggestions: contact
                .emails
                .into_iter()
                .enumerate()
                .map(|(idx, email)| Self {
                    key: format!("device-contact-email/{}-{}", contact.key, idx),
                    avatar_information: AvatarInformation::from(&contact.name),
                    name: contact.name.clone(),
                    kind: ContactSuggestionKind::DeviceContact(DeviceContactSuggestion { email }),
                })
                .collect(),
        }
    }
}

struct ContactGroup {
    key: String,
    name: String,
    emails: Vec<ContactEmailItem>,
}

#[derive(Debug)]
enum FollowingSuggestion {
    ContactGroup(ContactSuggestion),
    DeviceContact {
        name: String,
        key: String,
        suggestions: Vec<ContactSuggestion>,
    },
}

impl Ord for FollowingSuggestion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.lex_name()
            .cmp(&other.lex_name())
            .then(self.discriminant().cmp(&other.discriminant()))
            .then(self.key().cmp(other.key()))
    }
}
impl PartialEq for FollowingSuggestion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for FollowingSuggestion {}
impl PartialOrd for FollowingSuggestion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl FollowingSuggestion {
    fn lex_name(&self) -> String {
        let name = match self {
            FollowingSuggestion::ContactGroup(contact_suggestion) => &contact_suggestion.name,
            FollowingSuggestion::DeviceContact { name, .. } => name,
        };
        name.unicode_words().collect()
    }
    fn key(&self) -> &str {
        match self {
            FollowingSuggestion::ContactGroup(contact_suggestion) => {
                contact_suggestion.key.as_str()
            }
            FollowingSuggestion::DeviceContact { key, .. } => key.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContactSuggestionKind {
    ContactItem(ContactEmailItem),
    DeviceContact(DeviceContactSuggestion),
    ContactGroup(Vec<ContactEmailItem>),
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Hash)]
enum FollowingSuggestionDiscriminant {
    DeviceContact,
    ContactGroup,
}
impl FollowingSuggestion {
    fn discriminant(&self) -> FollowingSuggestionDiscriminant {
        match self {
            FollowingSuggestion::ContactGroup(_) => FollowingSuggestionDiscriminant::ContactGroup,
            FollowingSuggestion::DeviceContact { .. } => {
                FollowingSuggestionDiscriminant::DeviceContact
            }
        }
    }
}

/// A device, native contact, stored only locally on the current device.
///
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceContactSuggestion {
    pub email: PrivateEmail,
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;
    use crate::contact::Contact;
    use crate::contact_email::ContactEmail;
    use crate::test_utils::new_contact_test_connection;
    use mail_labels_common::{Label, LabelType, Labels};

    fn display_email_item(
        ContactEmailItem {
            local_contact_id,
            email,
            is_proton,
            last_used_time,
            name,
            avatar_information,
        }: ContactEmailItem,
        out: &mut String,
    ) {
        write!(
            out,
            "{} ({})",
            avatar_information.text, avatar_information.color
        )
        .unwrap();
        write!(out, ": {name} <{email}>").unwrap();
        if local_contact_id != 0.into() {
            write!(out, ",  local_contact_id: {local_contact_id}").unwrap();
        }
        if is_proton {
            write!(out, ", Proton address").unwrap();
        }
        if last_used_time.as_u64() != 0 {
            write!(out, ", last used: {last_used_time}").unwrap();
        }
        writeln!(out).unwrap();
    }

    fn display_suggestions(sug: Vec<ContactSuggestion>) -> String {
        let mut out = String::new();
        writeln!(out, "{} suggestions:", sug.len()).unwrap();
        for ContactSuggestion {
            key,
            name,
            avatar_information,
            kind,
        } in sug
        {
            writeln!(out, "\n{key}: {name}").unwrap();
            match kind {
                ContactSuggestionKind::ContactItem(em) => {
                    display_email_item(em, &mut out);
                }
                ContactSuggestionKind::DeviceContact(contact) => {
                    writeln!(
                        out,
                        "{} ({}): <{}>",
                        avatar_information.text, avatar_information.color, contact.email
                    )
                    .unwrap();
                }
                ContactSuggestionKind::ContactGroup(items) => {
                    for item in items {
                        display_email_item(item, &mut out);
                    }
                }
            }
        }
        out
    }

    fn display_group(groups: Vec<GroupedContacts>) -> String {
        let mut out = String::new();
        writeln!(out, "{} keys:", groups.len()).unwrap();
        for GroupedContacts { grouped_by, items } in groups {
            writeln!(
                out,
                "\n{grouped_by} ({} {})",
                items.len(),
                if items.len() == 1 { "item" } else { "items" }
            )
            .unwrap();
            for item in items {
                match item {
                    ContactItemType::Contact(ContactItem {
                        local_id: _,
                        name,
                        avatar_information,
                        emails,
                    }) => {
                        write!(
                            out,
                            "Contact {} ({}): {}",
                            avatar_information.text, avatar_information.color, name
                        )
                        .unwrap();
                        writeln!(
                            out,
                            " ({} {})",
                            emails.len(),
                            if emails.len() == 1 {
                                "address"
                            } else {
                                "addresses"
                            }
                        )
                        .unwrap();
                        for em in emails {
                            display_email_item(em, &mut out);
                        }
                    }
                    ContactItemType::Group(ContactGroupItem {
                        local_id: _,
                        name,
                        avatar_information,
                        contacts,
                    }) => {
                        write!(
                            out,
                            "Group {} ({}): {}",
                            avatar_information.text, avatar_information.color, name
                        )
                        .unwrap();
                        writeln!(
                            out,
                            " ({} {})",
                            contacts.len(),
                            if contacts.len() == 1 {
                                "address"
                            } else {
                                "addresses"
                            }
                        )
                        .unwrap();
                        for em in contacts {
                            display_email_item(em, &mut out);
                        }
                    }
                }
            }
        }
        out
    }

    mod contact_list {
        use mail_core_api::services::proton::LabelId;
        use mail_stash::orm::Model;
        use mail_stash::stash::StashError;
        use pretty_assertions::assert_eq;
        use test_case::test_case;

        use super::*;

        #[test_case(vec![], vec![]
        ,0; "TEST 0 Empty")]
        #[test_case(vec![crate::contact!(local_id: crate::lid!(123), name: "Barbara Lox".to_string())], vec![]
        ,1; "TEST 1 Basic")]
        #[test_case(vec![
            crate::contact!(local_id: crate::lid!(123), name: "Barbara Lox".to_string()),
            crate::contact!(local_id: crate::lid!(123), name: "Barbara Fox".to_string())
        ],
            vec![]
        ,2; "TEST 2 Alphabetical order")]
        #[test_case(vec![
            crate::contact!(local_id: crate::lid!(123), name: "🐂 Barbara Lox".to_string()),
            crate::contact!(local_id: crate::lid!(123), name: "🦊 Barbara Fox".to_string())
        ], vec![]
        ,3; "TEST 3 With emojis")]
        #[test_case(vec![
            crate::contact!(local_id: crate::lid!(123), name: "🙀".to_string()),
            crate::contact!(local_id: crate::lid!(123), name: "🙀 Barbara Lox".to_string()),
            crate::contact!(local_id: crate::lid!(123), name: "❤️‍🔥 Barbara Fox".to_string())
        ], vec![]
        ,4 ; "TEST 4 Mutliple groups")]
        #[test_case(vec![
            crate::contact!(local_id: crate::lid!(123), label_ids: crate::labels!("family"), name: "Mom".to_string()),
            crate::contact!(local_id: crate::lid!(124), label_ids: crate::labels!("family"), name: "Dad".to_string()),
            crate::contact!(local_id: crate::lid!(125), label_ids: crate::labels!("family"), name: "Sister".to_string())
        ], vec![
            crate::label!(local_id: crate::lid!(100), remote_id: Some(crate::label_id!("family")), name: "Family".to_string(), label_type: LabelType::ContactGroup)
        ]
        ,5; "TEST 5 Contact groups (labels)")]
        #[test_case(vec![
            crate::contact!(local_id: crate::lid!(123), name: "Jake Peralta".to_string(), contact_emails: vec![
                crate::contact_email!(local_id: crate::lid!(1), remote_id: crate::ceid!("1"), email: "jake@99.com".into(), label_ids: crate::labels!("squad")),
                crate::contact_email!(local_id: crate::lid!(2), remote_id: crate::ceid!("2"), email: "jake.peralta@work.com".into()),
            ]),
            crate::contact!(local_id: crate::lid!(124), name: "Amy Santiago".to_string(), contact_emails: vec![
                crate::contact_email!(local_id: crate::lid!(3), remote_id: crate::ceid!("3"), email: "amy@99.com".into(), label_ids: crate::labels!("squad")),
            ]),
        ], vec![
            crate::label!(local_id: crate::lid!(200), remote_id: Some(crate::label_id!("squad")), name: "Squad".to_string(), label_type: LabelType::ContactGroup)
        ]
        ,6; "TEST 6 Only emails explicitly added to the group are shown")]
        fn test_grouped_contacts(contacts: Vec<Contact>, groups: Vec<Label>, test_number: u32) {
            let groups = GroupedContacts::from_contacts_and_groups(contacts, groups);
            insta::assert_snapshot!(
                format!("test_grouped_contacts_{}", test_number),
                display_group(groups)
            );
        }

        #[tokio::test]
        async fn grouped_contacts_emails_order() {
            let emails = vec![
                crate::contact_email!(remote_id: crate::ceid!("3"), email: "barbara1984@yahoo.com".into(), display_order: 3),
                crate::contact_email!(remote_id: crate::ceid!("1"), email: "barbara@fox.us".into(), display_order: 2),
                crate::contact_email!(remote_id: crate::ceid!("2"), email: "bfox@proton.me".into(), display_order: 1, is_proton: true),
            ];

            let mut tether = new_contact_test_connection()
                .await
                .connection()
                .await
                .unwrap();
            let mut contact =
                crate::contact!(remote_id: crate::cid!("123"), name: "Barbara Fox".to_string());
            tether
                .tx::<_, _, StashError>(async |tx| {
                    contact.save(tx).await?;
                    for mut email in emails {
                        email.remote_contact_id = contact.remote_id.clone();
                        email.save(tx).await?;
                    }
                    Ok(())
                })
                .await
                .expect("commit failed");

            let result = Contact::contact_list(&tether).await.unwrap();
            insta::assert_snapshot!(display_group(result));
        }

        #[tokio::test]
        async fn contact_group_by_id_only_returns_emails_in_group() {
            let mut tether = new_contact_test_connection()
                .await
                .connection()
                .await
                .unwrap();

            let group_id = LabelId::from("squad");
            let mut group = Label {
                remote_id: Some(group_id.clone()),
                name: "Squad".to_owned(),
                label_type: LabelType::ContactGroup,
                ..Label::test_default()
            };

            let mut contact = crate::contact!(remote_id: crate::cid!("peralta"), name: "Jake Peralta".to_string());
            let email_in_group = crate::contact_email!(
                remote_id: crate::ceid!("1"),
                email: "jake@99.com".into(),
                label_ids: Labels::new(vec![group_id.clone()]),
                remote_contact_id: contact.remote_id.clone()
            );
            let email_not_in_group = crate::contact_email!(
                remote_id: crate::ceid!("2"),
                email: "jake.peralta@work.com".into(),
                remote_contact_id: contact.remote_id.clone()
            );
            contact.contact_emails = vec![email_in_group, email_not_in_group];

            tether
                .tx::<_, _, StashError>(async |tx| {
                    group.save(tx).await.unwrap();
                    contact.save(tx).await.unwrap();
                    Ok(())
                })
                .await
                .unwrap();

            let result = Contact::contact_group_by_id(&tether, group.id())
                .await
                .unwrap();

            assert_eq!(result.contacts.len(), 1);
            assert_eq!(result.contacts[0].email.as_clear_text_str(), "jake@99.com");
        }

        #[tokio::test]
        async fn count_email_group_count() {
            let mut tether = new_contact_test_connection()
                .await
                .connection()
                .await
                .unwrap();

            let empty_group_id = LabelId::from("l1");
            let not_empty_group_id = LabelId::from("l2");
            let mut contact_group_empty = Label {
                remote_id: Some(empty_group_id.clone()),
                name: "contact_group_empty".to_owned(),
                label_type: LabelType::ContactGroup,
                ..Label::test_default()
            };
            let mut contact_group_not_empty = Label {
                remote_id: Some(not_empty_group_id.clone()),
                name: "contact_group_not_empty".to_owned(),
                label_type: LabelType::ContactGroup,
                ..Label::test_default()
            };

            let mut contact1 =
                crate::contact!(remote_id: crate::cid!("123"), name: "Barbara Fox".to_string());
            let mut contact2 =
                crate::contact!(remote_id: crate::cid!("456"), name: "Stevie Wonder".to_string());
            let mut contact1_email = crate::contact_email!(remote_id: crate::ceid!("ceid1"), label_ids: Labels::new(vec![not_empty_group_id.clone()]), remote_contact_id: contact1.remote_id.clone());
            let mut contact2_email = crate::contact_email!(remote_id: crate::ceid!("ceid2"), label_ids: Labels::new(vec![not_empty_group_id.clone()]), remote_contact_id: contact2.remote_id.clone());

            tether
                .tx::<_, _, StashError>(async |tx| {
                    contact_group_empty.save(tx).await.unwrap();
                    contact_group_not_empty.save(tx).await.unwrap();
                    contact1.save(tx).await.unwrap();
                    contact2.save(tx).await.unwrap();
                    contact1_email.save(tx).await.unwrap();
                    contact2_email.save(tx).await.unwrap();
                    Ok(())
                })
                .await
                .unwrap();

            assert_eq!(
                ContactEmail::count_in_contact_group(empty_group_id, &tether)
                    .await
                    .unwrap(),
                0
            );
            assert_eq!(
                ContactEmail::count_in_contact_group(not_empty_group_id, &tether)
                    .await
                    .unwrap(),
                2
            );
            assert_eq!(
                ContactEmail::count_in_contact_group_by_name(
                    "contact_group_empty".to_owned(),
                    &tether
                )
                .await
                .unwrap(),
                Some(0)
            );
            assert_eq!(
                ContactEmail::count_in_contact_group_by_name(
                    "contact_group_not_empty".to_owned(),
                    &tether
                )
                .await
                .unwrap(),
                Some(2)
            );
            assert_eq!(
                ContactEmail::count_in_contact_group_by_name(
                    "contact_group_unknown".to_owned(),
                    &tether
                )
                .await
                .unwrap(),
                None
            );
        }
    }

    mod contact_suggestions {
        use mail_stash::orm::Model;
        use test_case::test_case;

        use super::*;

        #[derive(Default)]
        struct TestCase {
            contacts: Vec<Contact>,
            contact_groups: Vec<Label>,
            device_contacts: Vec<DeviceContact>,
        }

        #[test_case(TestCase::default()
        ,0; "TEST 0 - Empty")]
        #[test_case(TestCase {
            contacts: vec![
                crate::contact!(name: "Barbara Lox".to_string(),
                    contact_emails: vec![crate::contact_email!(local_id: crate::lid!(123), is_proton: false, email: "barbara@lox.com".into(), last_used_time: 1.into())
                    ])],
            ..Default::default()
         }
        ,1; "TEST 1 - Single contact")]
        #[test_case(TestCase {
            contacts: vec![
                crate::contact!(name: "Barbara Lox".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(123), is_proton: false, email: "barbara@lox.com".into(), last_used_time: 1.into())
                ]),
                crate::contact!(name: "Michael Scott".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(234), is_proton: true, email: "m.scott@pm.me".into(), last_used_time: 1.into())
                ])
            ],
            ..Default::default()
         }
        ,2; "TEST 2 - Proton mails go first")]
        #[test_case(TestCase {
            contacts: vec![
                crate::contact!(name: "Barbara Lox".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(123), is_proton: true, email: "barbara@pm.me".into(), last_used_time: 1.into())
                ]),
                crate::contact!(name: "Michael Scott".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(234), is_proton: true, email: "m.scott@pm.me".into(), last_used_time: 2.into())
                ]),
                crate::contact!(name: "Jake Peralta".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(456), is_proton: false, email: "jake.peralta@99.com".into(), last_used_time: 3.into())
                ])
            ],
            ..Default::default()
         }
        ,3; "TEST 3 - Frequently used mails go first")]
        #[test_case(TestCase {
            contacts: vec![
                crate::contact!(name: "Barbara Lox".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(123), is_proton: true, email: "barbara@pm.me".into(), last_used_time: 1.into())
                ]),
                crate::contact!(name: "Michael Scott".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(234), is_proton: true, email: "m.scott@pm.me".into(), last_used_time: 2.into())
                ]),
                crate::contact!(name: "Jason Mendoza".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(678), is_proton: true, email: "jianyu.li@pm.me".into(), last_used_time: 2.into())
                ]),
                crate::contact!(name: "Jake Peralta".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(456), is_proton: false, email: "jake.peralta@99.com".into(), last_used_time: 3.into())
                ]),
            ],
            ..Default::default()
         }
        ,4; "TEST 4 - In the end lexicographic order is used")]
        #[test_case(TestCase {
            contacts: vec![
                crate::contact!(name: "Barbara Lox".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(123), is_proton: true, email: "barbara@pm.me".into(), last_used_time: 1.into())
                ]),
                crate::contact!(name: "Michael Scott".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(234), is_proton: true, email: "m.scott@pm.me".into(), last_used_time: 2.into(), label_ids: crate::labels!("m.schur.productions"))
                ]),
                crate::contact!(name: "Jason Mendoza".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(678), is_proton: true, email: "jianyu.li@pm.me".into(), last_used_time: 2.into(), label_ids: crate::labels!("m.schur.productions"))
                ]),
                crate::contact!(name: "Jake Peralta".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(456), is_proton: false, email: "jake.peralta@99.com".into(), last_used_time: 3.into(), label_ids: crate::labels!("m.schur.productions")),
                    crate::contact_email!(local_id: crate::lid!(112), is_proton: false, email: "harvey@jp.com".into(), last_used_time: 1.into())
                ]),
            ],
            contact_groups: vec![
                crate::label!(local_id: crate::lid!(910), remote_id: Some(crate::label_id!("m.schur.productions")), name: "M. Schur Productions".into(), label_type: LabelType::ContactGroup),
            ],
            ..Default::default()
         }
        ,5; "TEST 5 - Contact groups")]
        #[test_case(TestCase {
            contacts: vec![
                crate::contact!(name: "Barbara Lox".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(123), is_proton: true, email: "barbara@pm.me".into(), last_used_time: 1.into())
                ]),
                crate::contact!(name: "Michael Scott".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(234), is_proton: true, email: "m.scott@pm.me".into(), last_used_time: 2.into(), label_ids: crate::labels!("m.schur.productions"))
                ]),
                crate::contact!(name: "Jason Mendoza".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(678), is_proton: true, email: "jianyu.li@pm.me".into(), last_used_time: 2.into(), label_ids: crate::labels!("m.schur.productions"))
                ]),
                crate::contact!(name: "Jake Peralta".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(456), is_proton: false, email: "jake.peralta@99.com".into(), last_used_time: 3.into(), label_ids: crate::labels!("m.schur.productions")),
                    crate::contact_email!(local_id: crate::lid!(112), is_proton: false, email: "harvey@jp.com".into(), last_used_time: 1.into())
                ]),
            ],
            contact_groups: vec![
                crate::label!(local_id: crate::lid!(910), remote_id: Some(crate::label_id!("m.schur.productions")), name: "M. Schur Productions".into(), label_type: LabelType::ContactGroup),
            ],
            device_contacts: vec![
                crate::device_contact!(key: "000".to_string(), name: "Aunt Molly".to_string(), emails: vec![
                    "molly@family.com".into(),
                    "badass@aunt.com".into(),
                ])
            ]
         }
        ,6; "TEST 6 - Contact groups and device contacts are in the end, sorted by name")]
        #[test_case(TestCase {
            contacts: vec![
                crate::contact!(name: "Barbara Lox".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(123), is_proton: true, email: "barbara@pm.me".into(), last_used_time: 1.into())
                ]),
                crate::contact!(name: "Michael Scott".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(234), is_proton: true, email: "m.scott@pm.me".into(), last_used_time: 2.into(), label_ids: crate::labels!("m.schur.productions"))
                ]),
                crate::contact!(name: "Jason Mendoza".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(678), is_proton: true, email: "jianyu.li@pm.me".into(), last_used_time: 2.into(), label_ids: crate::labels!("m.schur.productions"))
                ]),
                crate::contact!(name: "Jake Peralta".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(456), is_proton: false, email: "jake.peralta@99.com".into(), last_used_time: 3.into(), label_ids: crate::labels!("m.schur.productions")),
                    crate::contact_email!(local_id: crate::lid!(112), is_proton: false, email: "harvey@jp.com".into(), last_used_time: 1.into())
                ]),
            ],
            contact_groups: vec![
                crate::label!(local_id: crate::lid!(910), remote_id: Some(crate::label_id!("m.schur.productions")), name: "M. Schur Productions".into(), label_type: LabelType::ContactGroup),
            ],
            device_contacts: vec![
                crate::device_contact!(key: "000".to_string(), name: "Aunt Molly".to_string(), emails: vec![
                    "molly@family.com".into(),
                ]),
                crate::device_contact!(key: "001".to_string(), name: "Aunt Molly".to_string(), emails: vec![
                    "badass@aunt.com".into(),
                ])
            ]
         }
        ,7; "TEST 7 - Device Contacts are sorted by name and ids")]
        #[test_case(TestCase {
            contacts: vec![
                crate::contact!(name: "Barbara Lox".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(123), is_proton: true, email: "barbara@pm.me".into(), last_used_time: 1.into())
                ]),
                crate::contact!(name: "Michael Scott".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(234), is_proton: true, email: "m.scott@pm.me".into(), last_used_time: 2.into(), label_ids: crate::labels!("m.schur.productions"))
                ]),
                crate::contact!(name: "Jason Mendoza".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(678), is_proton: true, email: "jianyu.li@pm.me".into(), last_used_time: 2.into(), label_ids: crate::labels!("m.schur.productions"))
                ]),
                crate::contact!(name: "Jake Peralta".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(456), is_proton: false, email: "jake.peralta@99.com".into(), last_used_time: 3.into(), label_ids: crate::labels!("m.schur.productions")),
                    crate::contact_email!(local_id: crate::lid!(112), is_proton: false, email: "harvey@jp.com".into(), last_used_time: 1.into())
                ]),
                crate::contact!(name: "Detective Peralta".to_string(), contact_emails: vec![
                    crate::contact_email!(local_id: crate::lid!(999), is_proton: false, email: "jake.peralta@99.com".into(), last_used_time: 3.into())
                ])
            ],
            contact_groups: vec![
                crate::label!(local_id: crate::lid!(910), remote_id: Some(crate::label_id!("m.schur.productions")), name: "M. Schur Productions".to_string(), label_type: LabelType::ContactGroup),
            ],
            device_contacts: vec![
                crate::device_contact!(key: "000".to_string(), name: "Aunt Molly".to_string(), emails: vec![
                    "molly@family.com".into(),
                ]),
                crate::device_contact!(key: "001".to_string(), name: "Aunt Molly".to_string(), emails: vec![
                    "badass@aunt.com".into(),
                ]),
                crate::device_contact!(key: "002".to_string(), name: "Boss".to_string(), emails: vec![
                    "m.scott@pm.me".into()
                ]),
                crate::device_contact!(key: "003".to_string(), name: "Aunt Molly (Copy)".to_string(), emails: vec![
                    "badass@aunt.com".into(),
                ]),
            ]
         }
        ,8; "TEST 8 - contacts are deduplicated")]
        fn test_contact_suggestions(test_case: TestCase, test_number: u32) {
            let res = ContactSuggestions::from_contacts_and_device_contacts(
                test_case.contacts,
                test_case.contact_groups,
                test_case.device_contacts,
            )
            .all()
            .to_vec();
            insta::assert_snapshot!(
                format!("test_contact_suggestions_{}", test_number),
                display_suggestions(res)
            );
        }

        #[test_case(ContactSuggestions::from(
                 vec![
                    ContactSuggestion {
                        key: "contact/234".to_string(),
                        name: "Michael Scott".to_string(),
                        avatar_information: AvatarInformation {
                            text: "M".to_string(),
                            color: "#213474".to_string()
                        },
                        kind: ContactSuggestionKind::ContactItem(ContactEmailItem {
                            name: "Michael Scott".to_string(),
                            avatar_information: AvatarInformation {
                                text: "M".to_string(),
                                color: "#213474".to_string()
                            },
                            local_contact_id: 234.into(),
                            email: "m.scott@pm.me".into(),
                            is_proton: true,
                            last_used_time: 2.into()
                        })
                    },
                    ContactSuggestion {
                        key: "contact/123".to_string(),
                        name: "Barbara Lox".to_string(),
                        avatar_information: AvatarInformation {
                            text: "B".to_string(),
                            color: "#A839A4".to_string()
                        },
                        kind: ContactSuggestionKind::ContactItem(ContactEmailItem {
                            name: "Barbara Lox".to_string(),
                            avatar_information: AvatarInformation {
                                text: "B".to_string(),
                                color: "#A839A4".to_string()
                            },
                            local_contact_id: 123.into(),
                            email: "barbara@pm.me".into(),
                            is_proton: true,
                            last_used_time: 1.into()
                        })
                    },
                ]
            ),
            ContactSuggestions::from(
                vec![
                   ContactSuggestion {
                       key: "contact/234".to_string(),
                       name: "Michael Scott".to_string(),
                       avatar_information: AvatarInformation {
                           text: "M".to_string(),
                           color: "#213474".to_string()
                       },
                       kind: ContactSuggestionKind::ContactItem(ContactEmailItem {
                           name: "Michael Scott".to_string(),
                           avatar_information: AvatarInformation {
                               text: "M".to_string(),
                               color: "#213474".to_string()
                           },
                           local_contact_id: 234.into(),
                           email: "m.scott@pm.me".into(),
                           is_proton: true,
                           last_used_time: 2.into()
                       })
                   },
                   ContactSuggestion {
                       key: "contact/123".to_string(),
                       name: "Barbara Lox".to_string(),
                       avatar_information: AvatarInformation {
                           text: "B".to_string(),
                           color: "#A839A4".to_string()
                       },
                       kind: ContactSuggestionKind::ContactItem(ContactEmailItem {
                           name: "Barbara Lox".to_string(),
                           avatar_information: AvatarInformation {
                               text: "B".to_string(),
                               color: "#A839A4".to_string()
                           },
                           local_contact_id: 123.into(),
                           email: "barbara@pm.me".into(),
                           is_proton: true,
                           last_used_time: 1.into()
                       })
                   },
               ]
            ), 0;
            "TEST0: Concat the same suggestions ends up in the initial list"
        )]
        #[test_case(ContactSuggestions::from(
                 vec![
                    ContactSuggestion {
                        key: "contact/235".to_string(),
                        name: "Michael Brogile".to_string(),
                        avatar_information: AvatarInformation {
                            text: "M".to_string(),
                            color: "#213474".to_string()
                        },
                        kind: ContactSuggestionKind::ContactItem(ContactEmailItem {
                            name: "Michael Brogile".to_string(),
                            avatar_information: AvatarInformation {
                                text: "M".to_string(),
                                color: "#213474".to_string()
                            },
                            local_contact_id: 234.into(),
                            email: "m.brogile@pm.me".into(),
                            is_proton: true,
                            last_used_time: 2.into()
                        })
                    },
                    ContactSuggestion {
                        key: "contact/123".to_string(),
                        name: "Barbara Lox".to_string(),
                        avatar_information: AvatarInformation {
                            text: "B".to_string(),
                            color: "#A839A4".to_string()
                        },
                        kind: ContactSuggestionKind::ContactItem(ContactEmailItem {
                            name: "Barbara Lox".to_string(),
                            avatar_information: AvatarInformation {
                                text: "B".to_string(),
                                color: "#A839A4".to_string()
                            },
                            local_contact_id: 123.into(),
                            email: "barbara@pm.me".into(),
                            is_proton: true,
                            last_used_time: 1.into()
                        })
                    },
                ]
            ),
             ContactSuggestions::from(
                 vec![
                    ContactSuggestion {
                        key: "contact/234".to_string(),
                        name: "Michael Scott".to_string(),
                        avatar_information: AvatarInformation {
                            text: "M".to_string(),
                            color: "#213474".to_string()
                        },
                        kind: ContactSuggestionKind::ContactItem(ContactEmailItem {
                            name: "Michael Scott".to_string(),
                            avatar_information: AvatarInformation {
                                text: "M".to_string(),
                                color: "#213474".to_string()
                            },
                            local_contact_id: 234.into(),
                            email: "m.scott@pm.me".into(),
                            is_proton: true,
                            last_used_time: 2.into()
                        })
                    },
                    ContactSuggestion {
                        key: "contact/123".to_string(),
                        name: "Barbara Lox".to_string(),
                        avatar_information: AvatarInformation {
                            text: "B".to_string(),
                            color: "#A839A4".to_string()
                        },
                        kind: ContactSuggestionKind::ContactItem(ContactEmailItem {
                            avatar_information: AvatarInformation {
                                text: "B".to_string(),
                                color: "#A839A4".to_string()
                            },
                            name: "Barbara Lox".into(),
                            local_contact_id: 123.into(),
                            email: "barbara@pm.me".into(),
                            is_proton: true,
                            last_used_time: 1.into()
                        })
                    },
                ]
             ), 1;
            "TEST1: Concat different suggestions are correctly deduplicated and sorted (other's at the end)"
        )]
        fn concat_contact_suggestions(
            mut one: ContactSuggestions,
            other: ContactSuggestions,
            test_number: u32,
        ) {
            one.concat(other);
            insta::assert_snapshot!(
                format!("concat_contact_suggestions_{}", test_number),
                display_suggestions(one.all().to_vec())
            );
        }

        fn pretty_assert_emails(expected: Vec<&'static str>) -> impl Fn(Vec<ContactSuggestion>) {
            move |actual| {
                let actual = actual
                    .into_iter()
                    .map(|suggestion| match suggestion.kind {
                        ContactSuggestionKind::ContactItem(contact_email_item) => format!(
                            "{} <{}>",
                            suggestion.name,
                            contact_email_item.email.as_clear_text_str()
                        ),
                        ContactSuggestionKind::DeviceContact(device_contact_suggestion) => {
                            format!(
                                "{} <{}>",
                                suggestion.name,
                                device_contact_suggestion.email.as_clear_text_str()
                            )
                        }
                        ContactSuggestionKind::ContactGroup(vec) => {
                            format!("{} ({} emails)", suggestion.name, vec.len())
                        }
                    })
                    .collect::<Vec<_>>();
                pretty_assertions::assert_eq!(actual, expected);
            }
        }

        fn filtering_test_case() -> TestCase {
            TestCase {
                contacts: vec![
                    crate::contact!(name: "Barbara Lox".to_string(), remote_id: crate::cid!("lox"), contact_emails: vec![
                        crate::contact_email!(remote_id: crate::ceid!("123"), is_proton: true, email: "barbara@pm.me".into(), last_used_time: 1.into())
                    ]),
                    crate::contact!(name: "Michael Scott".to_string(), remote_id: crate::cid!("scott"), contact_emails: vec![
                        crate::contact_email!(remote_id: crate::ceid!("234"), is_proton: true, email: "m.scott@pm.me".into(), last_used_time: 2.into(), label_ids: crate::labels!("m.schur.productions"))
                    ]),
                    crate::contact!(name: "Jason Mendoza".to_string(), remote_id: crate::cid!("mendoza"), contact_emails: vec![
                        crate::contact_email!(remote_id: crate::ceid!("678"), is_proton: true, email: "jianyu.li@pm.me".into(), last_used_time: 2.into(), label_ids: crate::labels!("m.schur.productions"))
                    ]),
                    crate::contact!(name: "Jake Peralta".to_string(), remote_id: crate::cid!("peralta"), contact_emails: vec![
                        crate::contact_email!(remote_id: crate::ceid!("456"), is_proton: false, email: "jake.peralta@99.com".into(), last_used_time: 3.into(), label_ids: crate::labels!("m.schur.productions")),
                        crate::contact_email!(remote_id: crate::ceid!("112"), is_proton: false, email: "harvey@jp.com".into(), last_used_time: 1.into())
                    ]),
                ],
                contact_groups: vec![
                    crate::label!(remote_id: Some(crate::label_id!("m.schur.productions")), name: "M. Schur Productions".to_string(), label_type: LabelType::ContactGroup),
                ],
                device_contacts: vec![
                    crate::device_contact!(key: "000".to_string(), name: "Aunt Molly".to_string(), emails: vec![
                        "molly@family.com".into(),
                    ]),
                    crate::device_contact!(key: "001".to_string(), name: "Molly".to_string(), emails: vec![
                        "badass@aunt.com".into(),
                    ]),
                ],
            }
        }

        #[test_case("pe", TestCase::default() => using pretty_assert_emails(vec![]) ; "TEST 0A - empty contact book")]
        #[test_case("", TestCase::default() => using pretty_assert_emails(vec![]) ; "TEST 0B - empty query")]
        #[test_case("", filtering_test_case() => using pretty_assert_emails(vec![]) ; "TEST 0C - empty query with non-empty contact book")]
        #[test_case("Lox", filtering_test_case() => using pretty_assert_emails(vec![ "Barbara Lox <barbara@pm.me>" ]) ; "TEST 1 - filtering by name")]
        #[test_case("lox", filtering_test_case() => using pretty_assert_emails(vec![ "Barbara Lox <barbara@pm.me>" ]) ; "TEST 2 - filtering case insensitive")]
        #[test_case("jianyu", filtering_test_case() => using pretty_assert_emails(vec![ "Jason Mendoza <jianyu.li@pm.me>" ]) ; "TEST 3 - filtering by email")]
        #[test_case("Jake", filtering_test_case() => using pretty_assert_emails(vec![ "Jake Peralta <jake.peralta@99.com>", "Jake Peralta <harvey@jp.com>" ]) ; "TEST 4 - filtering by name, contact has multiple emails")]
        #[test_case("Schur", filtering_test_case() => using pretty_assert_emails(vec![ "M. Schur Productions (3 emails)"]) ; "TEST 5 - filtering by name, contact group returned")]
        #[test_case("aunt", filtering_test_case() => using pretty_assert_emails(vec![
            "Aunt Molly <molly@family.com>",
            "Molly <badass@aunt.com>",
        ]) ; "TEST 6 - device contacts filtered by both name and email")]
        #[test_case("m", filtering_test_case() => using pretty_assert_emails(vec![
            "Jason Mendoza <jianyu.li@pm.me>",
            "Michael Scott <m.scott@pm.me>",
            "Barbara Lox <barbara@pm.me>",
            "Jake Peralta <jake.peralta@99.com>",
            "Jake Peralta <harvey@jp.com>",
            "Aunt Molly <molly@family.com>",
            "M. Schur Productions (3 emails)",
            "Molly <badass@aunt.com>",
        ]) ; "TEST 7 - finding all")]
        #[tokio::test]
        async fn contact_suggestions_filtering(
            query: &str,
            mut test_case: TestCase,
        ) -> Vec<ContactSuggestion> {
            let mut tether = new_contact_test_connection()
                .await
                .connection()
                .await
                .unwrap();
            tether
                .tx::<_, _, mail_stash::stash::StashError>(async |tx| {
                    for label in &mut test_case.contact_groups {
                        label.save(tx).await.unwrap();
                    }
                    for contact in &mut test_case.contacts {
                        contact.save(tx).await.unwrap();
                    }
                    Ok(())
                })
                .await
                .expect("commit failed");

            let suggestions = Contact::contact_suggestions(test_case.device_contacts, &tether)
                .await
                .unwrap();

            suggestions.filtered(query)
        }
    }
}
