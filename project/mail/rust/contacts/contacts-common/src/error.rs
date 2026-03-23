use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::PrivateEmail;
use mail_stash::stash::StashError;
use mail_vcard::VcardValidationError;
use thiserror::Error;

use crate::local_ids::LocalContactId;

#[derive(Debug, Error)]
pub enum ContactError {
    #[error("ContactCard not found for email: {0}")]
    CardNotFound(PrivateEmail),
    #[error("RemoteId not present for ContactCard for email: {0}")]
    ContactCardRemoteIdNotPresent(PrivateEmail),
    #[error("Contact not found for email: {0}")]
    FullContactNotFound(PrivateEmail),
    #[error("Validation: {0}")]
    Validation(#[from] VcardValidationError),
    #[error("Contact {0} does not have remote id")]
    ContactDoesNotHaveRemoteId(LocalContactId),
    #[error(transparent)]
    Api(#[from] ApiServiceError),
    #[error(transparent)]
    Stash(#[from] StashError),
}
