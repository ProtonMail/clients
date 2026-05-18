pub mod contact;
pub mod contact_card;
pub mod contact_details;
pub mod contact_email;
pub mod contact_group;
pub mod contact_list;
pub mod database;
pub mod db;
pub mod error;
pub mod events;
pub mod local_ids;
pub mod types;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

#[cfg(test)]
mod tests;
