pub mod contact;
pub mod contact_card;
pub mod contact_details;
pub mod contact_email;
pub mod contact_list;
pub mod db;
pub mod error;
pub mod local_ids;
pub mod types;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

#[cfg(any(test, feature = "test-utils"))]
pub use mail_labels_common::{label, label_id, labels, lid};
