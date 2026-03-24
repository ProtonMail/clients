pub mod contact;
pub mod contact_card;
pub mod contact_details;
pub mod contact_email;
pub mod contact_list;
pub mod crypto;
pub mod db;
pub mod error;
pub mod local_ids;
pub mod types;
pub(crate) mod vcard_crypto;

pub use crypto::{
    AddressKeysContactFetchPolicy, ContactCryptoError, public_address_keys_from_contacts,
};

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

#[cfg(any(test, feature = "test-utils"))]
pub use mail_labels_common::{label, label_id, labels, lid};

#[cfg(test)]
mod tests;
