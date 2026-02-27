//! Core related database for user accounts, sessions and info.
//!
//! The module provide 2 distinct connection types which can be used interchangeably. It is up
//! to the user of this crate to decide whether they wish to store the user info in the same
//! or separate databases.

pub mod account;
mod addresses;
mod contacts;
mod core;
pub mod migrations;

pub type ChangeSender<T> =
    flume::Sender<mail_stash::orm::ResultsetChange<T, <T as mail_stash::orm::Model>::IdType>>;

pub type ChangeReceiver<T> =
    flume::Receiver<mail_stash::orm::ResultsetChange<T, <T as mail_stash::orm::Model>::IdType>>;

pub use mail_sqlite3;
