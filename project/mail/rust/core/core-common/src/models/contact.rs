pub use mail_contacts_common::contact::*;

use std::sync::Arc;

use crate::CoreContextError;
use crate::actions::contacts::Delete as ContactsDelete;
use crate::datatypes::LocalContactId;
use mail_action_queue::queue::{ActionError, Queue, QueuedActionOutput};
use mail_core_api::session::Session;
use mail_stash::UserDb;
use mail_stash::stash::Stash;

use super::{InitializationError, InitializationWatcher, InitializedComponent, Label};

/// Initializes contacts by syncing with the backend.
///
/// This function is idempotent. If successfully initialized in the past it will
/// skip re-initialization.
#[allow(clippy::result_large_err)]
pub async fn initialize_contacts(
    watcher: Arc<InitializationWatcher>,
    api: &Session,
    mail_stash: &Stash<UserDb>,
) -> Result<(), InitializationError<CoreContextError>> {
    InitializedComponent::initialize(
        watcher,
        Contact::INIT_KEY,
        &[Label::INIT_KEY],
        mail_stash.connection(),
        async move || Ok(Contact::sync(api).await?),
        |tx, res| {
            res.store(tx)?;
            Ok(())
        },
    )
    .await
}

/// Queues a delete action for the given contacts.
pub async fn action_delete_contacts(
    queue: &Queue<UserDb>,
    contact_ids: Vec<LocalContactId>,
) -> Result<QueuedActionOutput<ContactsDelete, UserDb>, ActionError<ContactsDelete, UserDb>> {
    let action = ContactsDelete::new(contact_ids);
    queue.queue_action(action).await
}
