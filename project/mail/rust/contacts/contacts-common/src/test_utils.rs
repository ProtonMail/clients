use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashConfiguration};

pub async fn new_contact_test_connection() -> Stash<UserDb> {
    let stash = Stash::new(StashConfiguration::test()).unwrap();
    let mut tether = stash.connection();
    mail_labels_common::db::migrate(&mut tether).await.unwrap();
    crate::db::migrate(&mut tether).await.unwrap();
    drop(tether);
    stash
}

#[macro_export]
macro_rules! cid {
    ($id:expr) => {{
        use mail_core_api::services::proton::ContactId;
        Some(ContactId::from($id))
    }};
}

#[macro_export]
macro_rules! cgid {
    ($id:expr) => {{
        use mail_contacts_api::ContactGroupId;
        Some(ContactGroupId::from($id))
    }};
}

#[macro_export]
macro_rules! lcgid {
    ($id:expr) => {{
        use $crate::local_ids::LocalContactGroupId;
        Some(LocalContactGroupId::from($id))
    }};
}

#[macro_export]
macro_rules! lid {
    ($id:expr) => {{
        use $crate::local_ids::LocalContactId;
        Some(LocalContactId::from($id))
    }};
}

#[macro_export]
macro_rules! leid {
    ($id:expr) => {{
        use $crate::local_ids::LocalContactEmailId;
        Some(LocalContactEmailId::from($id))
    }};
}

#[macro_export]
macro_rules! rcgid {
    ($id:expr) => {{
        use mail_contacts_api::ContactGroupId;
        Some(ContactGroupId::from($id))
    }};
}

#[macro_export]
macro_rules! ceid {
    ($id:expr) => {{
        use mail_core_api::services::proton::ContactEmailId;
        Some(ContactEmailId::from($id))
    }};
}

#[macro_export]
macro_rules! contact {
    ($($field:tt)*) => {{
        $crate::contact::Contact {
            $($field)*,
            ..$crate::contact::Contact::test_default()
        }
    }};
}

#[macro_export]
macro_rules! contact_email {
    ($($field:tt)*) => {{
        $crate::contact_email::ContactEmail {
            $($field)*,
            ..$crate::contact_email::ContactEmail::test_default()
        }
    }};
}

#[macro_export]
macro_rules! device_contact {
    ($($field:tt)*) => {{
        #[allow(clippy::needless_update)]
        $crate::contact_list::DeviceContact {
            $($field)*,
            ..Default::default()
        }
    }};
}

#[macro_export]
macro_rules! contact_group_ids {
    ($($label:expr),*) => {{
        use mail_contacts_api::ContactGroupId;
        vec![$(
            ContactGroupId::from($label)
        ),*]
    }};
}

#[macro_export]
macro_rules! label {
    ($($field:tt)*) => {{
        $crate::contact_group::ContactGroup {
            $($field)*,
            ..$crate::contact_group::ContactGroup::test_default()
        }
    }};
}
