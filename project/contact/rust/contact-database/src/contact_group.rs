use contact_lattice::ContactGroupId;

use crate::LocalContactGroupId;

pub struct ContactGroup {
    pub local_id: LocalContactGroupId,
    pub remote_id: Option<ContactGroupId>,
    pub color: String,
    pub display: bool,
    pub name: String,
    pub order: u32,
    pub sticky: bool,
}

pub struct UpsertableContactGroup {
    pub id: ContactGroupId,
    pub color: String,
    pub display: bool,
    pub name: String,
    pub order: u32,
    pub sticky: bool,
}

pub struct NewContactGroup {
    pub color: String,
    pub display: bool,
    pub name: String,
    pub order: u32,
    pub sticky: bool,
}

pub trait RoContactGroupTable {
    type Error: std::error::Error + 'static;

    async fn find_contact_group_by_id(
        &self,
        id: LocalContactGroupId,
    ) -> Result<Option<ContactGroup>, Self::Error>;

    async fn find_contact_group_by_remote_id(
        &self,
        id: &ContactGroupId,
    ) -> Result<Option<ContactGroup>, Self::Error>;
}

pub trait RwContactGroupTable: RoContactGroupTable {
    async fn create_contact_group(
        &self,
        contact_email: NewContactGroup,
    ) -> Result<ContactGroup, Self::Error>;

    async fn upsert_contact_group(
        &self,
        contact_email: UpsertableContactGroup,
    ) -> Result<ContactGroup, Self::Error>;

    async fn upsert_contact_groups(
        &self,
        contact_email: impl IntoIterator<Item = UpsertableContactGroup>,
    ) -> Result<Vec<ContactGroup>, Self::Error>;

    async fn update_contact_group(&self, contact_email: &ContactGroup) -> Result<(), Self::Error>;

    async fn delete_contact_groups(
        &self,
        ids: impl IntoIterator<Item = LocalContactGroupId>,
    ) -> Result<(), Self::Error>;
}
