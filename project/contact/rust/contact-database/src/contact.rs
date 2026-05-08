use contact_lattice::{ContactGroupId, ContactId, ContactUID};

use crate::{LocalContactGroupId, LocalContactId};

#[derive(Debug, Clone)]
pub struct Contact {
    pub local_id: LocalContactId,
    pub remote_id: Option<ContactId>,
    pub create_time: u64,
    pub label_ids: Vec<LocalContactGroupId>,
    pub modify_time: u64,
    pub name: String,
    pub size: u64,
    pub uid: ContactUID,
    /// Reflects whether the record has been deleted. This is used to ensure that
    /// delete happens in a two-step process, where the record is marked as
    /// deleted, then deleted from remote, then finally deleted from the local
    /// by event loop update.
    pub deleted: bool,
}

pub struct UpsertableContact {
    pub id: ContactId,
    pub create_time: u64,
    pub label_ids: Vec<ContactGroupId>,
    pub modify_time: u64,
    pub name: String,
    pub size: u64,
    pub uid: ContactUID,
}

pub struct NewContact {
    pub create_time: u64,
    pub label_ids: Vec<ContactGroupId>,
    pub modify_time: u64,
    pub name: String,
    pub size: u64,
    pub uid: ContactUID,
}

pub trait RoContactTable {
    type Error: std::error::Error + 'static;
    async fn find_contact_by_id(&self, id: LocalContactId) -> Result<Option<Contact>, Self::Error>;
    async fn find_contact_by_remote_id(
        &self,
        id: &ContactId,
    ) -> Result<Option<Contact>, Self::Error>;
    async fn find_contacts_by_ids(
        &self,
        id: impl IntoIterator<Item = LocalContactId>,
    ) -> Result<Vec<Contact>, Self::Error>;
    async fn find_contact_by_remote_ids(&self, id: ContactId) -> Result<Vec<Contact>, Self::Error>;
}

pub trait RwContactTable: RoContactTable {
    async fn create_contact(&self, contact: NewContact) -> Result<Contact, Self::Error>;
    async fn upsert_contact(&self, contact: UpsertableContact) -> Result<Contact, Self::Error>;
    async fn upsert_contacts(
        &self,
        contacts: impl IntoIterator<Item = UpsertableContact>,
    ) -> Result<Vec<Contact>, Self::Error>;
    async fn update_contact(&self, contact: &Contact) -> Result<(), Self::Error>;
    async fn mark_contact_as_deleted(
        &self,
        ids: impl IntoIterator<Item = LocalContactId>,
    ) -> Result<(), Self::Error>;
    async fn mark_contact_as_undeleted(
        &self,
        ids: impl IntoIterator<Item = LocalContactId>,
    ) -> Result<(), Self::Error>;
    async fn delete_contacts(
        &self,
        ids: impl IntoIterator<Item = LocalContactId>,
    ) -> Result<(), Self::Error>;
}
