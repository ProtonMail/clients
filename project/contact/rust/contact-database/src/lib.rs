#![allow(async_fn_in_trait)]
mod contact;
mod contact_card;
mod contact_email;
mod contact_group;

pub use contact::*;
pub use contact_card::*;
pub use contact_email::*;
pub use contact_group::*;

use contact_lattice::{ContactEmailId, ContactGroupId, ContactId};
use mail_local_id::declare_local_id;

declare_local_id!(LocalContactId => ContactId);
declare_local_id!(LocalContactGroupId => ContactGroupId);
declare_local_id!(LocalContactEmailId => ContactEmailId);
