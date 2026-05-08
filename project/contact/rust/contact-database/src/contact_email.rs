use contact_lattice::{ContactEmailId, ContactGroupId, ContactId, ContactSendingPreferences};
use mail_proton_ids::PrivateEmail;

use crate::{LocalContactEmailId, LocalContactGroupId, LocalContactId};

#[derive(Debug, Clone)]
pub struct ContactEmail {
    pub local_id: LocalContactEmailId,
    pub remote_id: Option<ContactEmailId>,
    pub local_contact_id: LocalContactId,
    pub canonical_email: PrivateEmail,
    pub contact_type: Vec<String>,
    pub defaults: ContactSendingPreferences,
    pub display_order: u32,
    pub email: PrivateEmail,
    pub is_proton: bool,
    pub label_ids: Vec<LocalContactGroupId>,
    pub last_used_time: u64,
    pub name: String,
}

pub struct UpseratableContactEmail {
    pub id: ContactEmailId,
    pub contact_id: ContactId,
    pub canonical_email: PrivateEmail,
    pub contact_type: Vec<String>,
    pub defaults: ContactSendingPreferences,
    pub display_order: u32,
    pub email: PrivateEmail,
    pub is_proton: bool,
    pub label_ids: Vec<ContactGroupId>,
    pub last_used_time: u64,
    pub name: String,
}

pub struct NewContactEmail {
    pub contact_id: LocalContactId,
    pub canonical_email: PrivateEmail,
    pub contact_type: Vec<String>,
    pub defaults: ContactSendingPreferences,
    pub display_order: u32,
    pub email: PrivateEmail,
    pub is_proton: bool,
    pub label_ids: Vec<LocalContactGroupId>,
    pub last_used_time: u64,
    pub name: String,
}

pub trait RoContactEmailTable {
    type Error: std::error::Error + 'static;

    async fn find_contact_email_by_id(
        &self,
        id: LocalContactEmailId,
    ) -> Result<Option<ContactEmail>, Self::Error>;

    async fn find_contact_email_by_remote_id(
        &self,
        id: &ContactEmailId,
    ) -> Result<Option<ContactEmail>, Self::Error>;

    async fn count_contact_emails_in_group_by_name(&self, name: &str)
    -> Result<usize, Self::Error>;

    async fn count_contact_emails_in_group(
        &self,
        contact_group_id: LocalContactGroupId,
    ) -> Result<usize, Self::Error>;

    async fn count_contact_emails_in_group_with_remote_id(
        &self,
        contact_group_id: &ContactGroupId,
    ) -> Result<usize, Self::Error>;
}

pub trait RwContactEmailTable: RoContactEmailTable {
    async fn crate_contact_email(
        &self,
        contact_email: NewContactEmail,
    ) -> Result<ContactEmail, Self::Error>;

    async fn upsert_contact_email(
        &self,
        contact_email: UpseratableContactEmail,
    ) -> Result<ContactEmail, Self::Error>;

    async fn upsert_contact_emails(
        &self,
        contact_emails: impl IntoIterator<Item = UpseratableContactEmail>,
    ) -> Result<Vec<ContactEmail>, Self::Error>;

    async fn update_contact_email(&self, contact_email: &ContactEmail) -> Result<(), Self::Error>;

    async fn delete_contact_emails(
        &self,
        ids: impl IntoIterator<Item = LocalContactEmailId>,
    ) -> Result<(), Self::Error>;
}
