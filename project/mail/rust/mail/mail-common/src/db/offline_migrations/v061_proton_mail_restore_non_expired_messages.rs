use indoc::indoc;
use mail_sqlite3::Migration;
use mail_stash::stash::{StashError, WriteTx};
use mail_stash::{UserDb, params};

use crate::datatypes::LocalMessageId;
use crate::models::Message;

pub struct RestoreNonExpiredMessages;

#[async_trait::async_trait]
impl Migration<UserDb> for RestoreNonExpiredMessages {
    fn name(&self) -> &str {
        "v061_proton_mail_restore_non_expired_messages"
    }

    async fn migrate(&self, tx: &WriteTx<'_>) -> Result<(), StashError> {
        // Attempt to undelted all messages that belong to an expired conversation that have not
        // really expired.
        let ids:Vec<LocalMessageId> = tx.query_values(indoc! {"
            SELECT local_id FROM messages 
                WHERE expiration_time=0 AND deleted=1 AND local_conversation_id IN (
                    SELECT local_id FROM conversations WHERE expiration_time <> 0 AND expiration_time < STRFTIME('%s', 'NOW')
                )"}, params![]).await?;
        if ids.is_empty() {
            return Ok(());
        }
        if let Err(e) = Message::mark_undeleted(ids, tx).await {
            match e {
                crate::AppError::Stash(stash_error) => return Err(stash_error),
                // if we have another error that is raised, we don't want to block the migration
                // from succeeding.
                e => {
                    tracing::error!("Failed to undelete meessges: {e}");
                }
            }
        }
        Ok(())
    }
}
