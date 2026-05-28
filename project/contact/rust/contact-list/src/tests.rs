use std::fmt::Write as _;

use contact_database::{
    Contact, ContactEmail, ContactGroup, LocalContactEmailId, LocalContactGroupId, LocalContactId,
};
use contact_lattice::{ContactGroupId, ContactSendingPreferences, ContactUID};
use test_case::test_case;

use crate::{ContactEmailItem, ContactGroupItem, ContactItem, ContactItemType, GroupedContacts};

fn contact(local_id: u64, name: &str) -> Contact {
    Contact {
        local_id: LocalContactId::from(local_id),
        remote_id: None,
        create_time: 0,
        label_ids: vec![],
        modify_time: 0,
        name: name.to_string(),
        size: 0,
        uid: ContactUID::from("uid"),
        deleted: false,
    }
}

fn email(local_id: u64, contact_id: u64, address: &str) -> ContactEmail {
    ContactEmail {
        local_id: LocalContactEmailId::from(local_id),
        remote_id: None,
        local_contact_id: LocalContactId::from(contact_id),
        canonical_email: address.into(),
        contact_type: vec![],
        defaults: ContactSendingPreferences::Default,
        display_order: 0,
        email: address.into(),
        is_proton: false,
        label_ids: vec![],
        last_used_time: 0,
        name: String::new(),
    }
}

fn email_in_group(
    local_id: u64,
    contact_id: u64,
    address: &str,
    group_local_id: u64,
) -> ContactEmail {
    let mut e = email(local_id, contact_id, address);
    e.label_ids = vec![LocalContactGroupId::from(group_local_id)];
    e
}

fn group(local_id: u64, remote_id: &str, name: &str) -> ContactGroup {
    ContactGroup {
        local_id: LocalContactGroupId::from(local_id),
        remote_id: Some(ContactGroupId::from(remote_id)),
        color: String::new(),
        display: true,
        name: name.to_string(),
        order: 0,
        sticky: false,
    }
}

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
    if last_used_time != 0 {
        write!(out, ", last used: {last_used_time}").unwrap();
    }
    writeln!(out).unwrap();
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

fn case_empty() -> (Vec<(Contact, Vec<ContactEmail>)>, Vec<ContactGroup>) {
    (vec![], vec![])
}

fn case_basic() -> (Vec<(Contact, Vec<ContactEmail>)>, Vec<ContactGroup>) {
    (vec![(contact(123, "Barbara Lox"), vec![])], vec![])
}

fn case_alphabetical() -> (Vec<(Contact, Vec<ContactEmail>)>, Vec<ContactGroup>) {
    (
        vec![
            (contact(123, "Barbara Lox"), vec![]),
            (contact(123, "Barbara Fox"), vec![]),
        ],
        vec![],
    )
}

fn case_emojis() -> (Vec<(Contact, Vec<ContactEmail>)>, Vec<ContactGroup>) {
    (
        vec![
            (contact(123, "\u{1F402} Barbara Lox"), vec![]),
            (contact(123, "\u{1F98A} Barbara Fox"), vec![]),
        ],
        vec![],
    )
}

fn case_multiple_groups() -> (Vec<(Contact, Vec<ContactEmail>)>, Vec<ContactGroup>) {
    (
        vec![
            (contact(123, "\u{1F640}"), vec![]),
            (contact(123, "\u{1F640} Barbara Lox"), vec![]),
            (
                contact(123, "\u{2764}\u{FE0F}\u{200D}\u{1F525} Barbara Fox"),
                vec![],
            ),
        ],
        vec![],
    )
}

fn case_groups_no_emails() -> (Vec<(Contact, Vec<ContactEmail>)>, Vec<ContactGroup>) {
    (
        vec![
            (contact(123, "Mom"), vec![]),
            (contact(124, "Dad"), vec![]),
            (contact(125, "Sister"), vec![]),
        ],
        vec![group(100, "family", "Family")],
    )
}

fn case_emails_in_group() -> (Vec<(Contact, Vec<ContactEmail>)>, Vec<ContactGroup>) {
    (
        vec![
            (
                contact(123, "Jake Peralta"),
                vec![
                    email_in_group(1, 123, "jake@99.com", 200),
                    email(2, 123, "jake.peralta@work.com"),
                ],
            ),
            (
                contact(124, "Amy Santiago"),
                vec![email_in_group(3, 124, "amy@99.com", 200)],
            ),
        ],
        vec![group(200, "squad", "Squad")],
    )
}

#[test_case(case_empty(), 0; "TEST 0 Empty")]
#[test_case(case_basic(), 1; "TEST 1 Basic")]
#[test_case(case_alphabetical(), 2; "TEST 2 Alphabetical order")]
#[test_case(case_emojis(), 3; "TEST 3 With emojis")]
#[test_case(case_multiple_groups(), 4; "TEST 4 Multiple groups")]
#[test_case(case_groups_no_emails(), 5; "TEST 5 Contact groups (labels)")]
#[test_case(case_emails_in_group(), 6; "TEST 6 Only emails explicitly added to the group are shown")]
fn test_grouped_contacts(
    case: (Vec<(Contact, Vec<ContactEmail>)>, Vec<ContactGroup>),
    test_number: u32,
) {
    let (contacts, groups) = case;
    let result = GroupedContacts::from_contacts_and_groups(contacts, groups);
    insta::assert_snapshot!(
        format!("test_grouped_contacts_{test_number}"),
        display_group(result)
    );
}
