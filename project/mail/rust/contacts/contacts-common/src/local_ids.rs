use mail_core_api::services::proton::{ContactEmailId, ContactId};
use mail_shared_types::declare_local_id;

declare_local_id!(LocalContactId => ContactId);
declare_local_id!(LocalContactEmailId => ContactEmailId);
