use mail_contacts_api::ContactGroupId;
use mail_core_api::services::proton::{ContactEmailId, ContactId};
use mail_shared_types::declare_local_id;

declare_local_id!(LocalContactId => ContactId);
declare_local_id!(LocalContactEmailId => ContactEmailId);
declare_local_id!(LocalContactGroupId => ContactGroupId);
declare_local_id!(LocalContactCardId);

impl From<LocalContactId> for contact_database::LocalContactId {
    fn from(value: LocalContactId) -> Self {
        Self::from(value.0)
    }
}

impl From<contact_database::LocalContactId> for LocalContactId {
    fn from(value: contact_database::LocalContactId) -> Self {
        Self::from(value.as_u64())
    }
}

impl From<LocalContactEmailId> for contact_database::LocalContactEmailId {
    fn from(value: LocalContactEmailId) -> Self {
        Self::from(value.0)
    }
}

impl From<contact_database::LocalContactEmailId> for LocalContactEmailId {
    fn from(value: contact_database::LocalContactEmailId) -> Self {
        Self::from(value.as_u64())
    }
}

impl From<LocalContactGroupId> for contact_database::LocalContactGroupId {
    fn from(value: LocalContactGroupId) -> Self {
        Self::from(value.0)
    }
}

impl From<contact_database::LocalContactGroupId> for LocalContactGroupId {
    fn from(value: contact_database::LocalContactGroupId) -> Self {
        Self::from(value.as_u64())
    }
}

impl From<LocalContactCardId> for contact_database::LocalContactCardId {
    fn from(value: LocalContactCardId) -> Self {
        Self::from(value.0)
    }
}

impl From<contact_database::LocalContactCardId> for LocalContactCardId {
    fn from(value: contact_database::LocalContactCardId) -> Self {
        Self::from(value.as_u64())
    }
}
