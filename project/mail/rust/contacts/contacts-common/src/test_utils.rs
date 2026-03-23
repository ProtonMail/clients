use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashConfiguration};

pub async fn new_contact_test_connection() -> Stash<UserDb> {
    let stash = Stash::new(StashConfiguration::test()).unwrap();
    let mut tether = stash.connection().await.unwrap();
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
