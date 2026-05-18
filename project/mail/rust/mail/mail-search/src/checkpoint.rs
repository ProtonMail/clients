//! SQLite persistence for ephemeral historic-load pagination anchors (All Mail only).

use mail_api::services::proton::common::MessageId;
use mail_stash::UserDb;
use mail_stash::stash::Stash;

use crate::service::{MailSearchService, SearchServiceError};

/// Fixed primary key for the singleton checkpoint row (same pattern as `user_settings`).
const CHECKPOINT_ROW_ID: i64 = 1;

/// Saved anchor: next run fetches messages older than this pair (All Mail, server descending-time order).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EphemeralHistoricCheckpoint {
    pub anchor_time: u64,
    pub anchor_message_id: MessageId,
}

impl MailSearchService {
    /// Load the saved All Mail checkpoint, if any.
    pub async fn load_ephemeral_historic_checkpoint(
        &self,
    ) -> Result<Option<EphemeralHistoricCheckpoint>, SearchServiceError> {
        load_checkpoint(self.mail_stash()).await
    }

    /// Persist the anchor after a successful batch (overwrites any previous checkpoint).
    pub async fn save_ephemeral_historic_checkpoint(
        &self,
        anchor_time: u64,
        anchor_message_id: &MessageId,
    ) -> Result<(), SearchServiceError> {
        save_checkpoint(self.mail_stash(), anchor_time, anchor_message_id).await
    }

    /// Remove the checkpoint (e.g. end of mailbox or fresh start).
    pub async fn clear_ephemeral_historic_checkpoint(&self) -> Result<(), SearchServiceError> {
        clear_checkpoint(self.mail_stash()).await
    }
}

async fn load_checkpoint(
    mail_stash: &Stash<UserDb>,
) -> Result<Option<EphemeralHistoricCheckpoint>, SearchServiceError> {
    let tether = mail_stash.connection();
    let row = tether
        .sync_query(move |conn| {
            use mail_stash::rusqlite::OptionalExtension;

            conn.query_row(
                "SELECT anchor_time, anchor_message_id FROM ephemeral_historic_load_checkpoint WHERE id = ?1",
                [CHECKPOINT_ROW_ID],
                |row| {
                    let anchor_time: i64 = row.get(0)?;
                    let anchor_message_id: String = row.get(1)?;
                    Ok((anchor_time, anchor_message_id))
                },
            )
            .optional()
            .map_err(StashError::from)
        })
        .await
        .map_err(map_stash_err)?;

    match row {
        None => Ok(None),
        Some((anchor_time, anchor_message_id)) => Ok(Some(EphemeralHistoricCheckpoint {
            anchor_time: i64_to_anchor_time(anchor_time)?,
            anchor_message_id: MessageId::from(anchor_message_id),
        })),
    }
}

async fn save_checkpoint(
    mail_stash: &Stash<UserDb>,
    anchor_time: u64,
    anchor_message_id: &MessageId,
) -> Result<(), SearchServiceError> {
    let anchor_message_id = anchor_message_id.as_str().to_owned();
    let anchor_time_i64 = u64_to_anchor_time_i64(anchor_time)?;
    let updated_at = chrono::Utc::now().timestamp();

    let mut tether = mail_stash.connection();
    tether
        .sync_write_tx(move |tx| {
            tx.execute(
                "INSERT INTO ephemeral_historic_load_checkpoint (id, anchor_time, anchor_message_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                   anchor_time = excluded.anchor_time,
                   anchor_message_id = excluded.anchor_message_id,
                   updated_at = excluded.updated_at",
                (CHECKPOINT_ROW_ID, anchor_time_i64, anchor_message_id, updated_at),
            )?;
            Ok(())
        })
        .await
        .map_err(map_stash_err)
}

async fn clear_checkpoint(mail_stash: &Stash<UserDb>) -> Result<(), SearchServiceError> {
    let mut tether = mail_stash.connection();
    tether
        .sync_write_tx(move |tx| {
            tx.execute(
                "DELETE FROM ephemeral_historic_load_checkpoint WHERE id = ?1",
                (CHECKPOINT_ROW_ID,),
            )?;
            Ok(())
        })
        .await
        .map_err(map_stash_err)
}

fn i64_to_anchor_time(value: i64) -> Result<u64, SearchServiceError> {
    u64::try_from(value)
        .map_err(|_| SearchServiceError::Checkpoint(format!("anchor_time out of range: {value}")))
}

fn u64_to_anchor_time_i64(value: u64) -> Result<i64, SearchServiceError> {
    i64::try_from(value)
        .map_err(|_| SearchServiceError::Checkpoint(format!("anchor_time out of range: {value}")))
}

use mail_stash::stash::StashError;

fn map_stash_err(e: StashError) -> SearchServiceError {
    SearchServiceError::Checkpoint(e.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mail_api::services::proton::common::MessageId;
    use mail_stash::stash::{Stash, StashConfiguration};
    use mail_task_service::TaskService;

    use crate::MailSearchService;

    async fn test_search_service() -> MailSearchService {
        let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
        crate::migrations::run(&mail_stash).await.unwrap();
        let task_service = Arc::new(
            TaskService::new(tokio::runtime::Handle::current())
                .expect("Failed to create TaskService"),
        );
        MailSearchService::new(mail_stash, task_service)
            .await
            .expect("MailSearchService::new")
    }

    #[tokio::test]
    async fn checkpoint_table_exists_after_migration() {
        let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
        crate::migrations::run(&mail_stash).await.unwrap();

        let tether = mail_stash.connection();
        let exists: bool = tether
            .sync_query(|conn| {
                use mail_stash::rusqlite::OptionalExtension;
                conn.query_row(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'ephemeral_historic_load_checkpoint' LIMIT 1",
                    [],
                    |_| Ok(1),
                )
                .optional()
                .map(|opt| opt.is_some())
                .map_err(mail_stash::stash::StashError::from)
            })
            .await
            .unwrap();
        assert!(
            exists,
            "ephemeral_historic_load_checkpoint table should exist"
        );
    }

    #[tokio::test]
    async fn checkpoint_roundtrip_and_clear() {
        let svc = test_search_service().await;

        assert_eq!(
            svc.load_ephemeral_historic_checkpoint().await.unwrap(),
            None
        );

        let anchor_id = MessageId::from(
            "A8DXH6a1Ap8PxVC6mJuiKjHXIulF3EZAzWjx6614h4tWBw5UFjZ2DXc5SLmyDjJhWKzyYdpRchgFeMeiw40SBw==",
        );
        svc.save_ephemeral_historic_checkpoint(1_778_796_668, &anchor_id)
            .await
            .unwrap();

        let loaded = svc
            .load_ephemeral_historic_checkpoint()
            .await
            .unwrap()
            .expect("checkpoint should exist");
        assert_eq!(loaded.anchor_time, 1_778_796_668);
        assert_eq!(loaded.anchor_message_id, anchor_id);

        svc.clear_ephemeral_historic_checkpoint().await.unwrap();
        assert_eq!(
            svc.load_ephemeral_historic_checkpoint().await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn checkpoint_save_overwrites_previous_anchor() {
        let svc = test_search_service().await;

        svc.save_ephemeral_historic_checkpoint(
            1_778_796_668,
            &MessageId::from("older-batch-boundary-a"),
        )
        .await
        .unwrap();
        svc.save_ephemeral_historic_checkpoint(
            1_778_764_317,
            &MessageId::from("older-batch-boundary-b"),
        )
        .await
        .unwrap();

        let loaded = svc
            .load_ephemeral_historic_checkpoint()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.anchor_time, 1_778_764_317);
        assert_eq!(
            loaded.anchor_message_id,
            MessageId::from("older-batch-boundary-b")
        );
    }

    #[tokio::test]
    async fn clear_index_tables_clears_checkpoint() {
        let mail_stash = Stash::new(StashConfiguration::test()).unwrap();
        crate::migrations::run(&mail_stash).await.unwrap();
        let task_service = Arc::new(
            TaskService::new(tokio::runtime::Handle::current())
                .expect("Failed to create TaskService"),
        );
        let svc = MailSearchService::new(mail_stash, task_service.clone())
            .await
            .unwrap();

        svc.save_ephemeral_historic_checkpoint(99, &MessageId::from("before-clear"))
            .await
            .unwrap();
        assert!(
            svc.load_ephemeral_historic_checkpoint()
                .await
                .unwrap()
                .is_some()
        );

        svc.clear_index_tables(task_service).await.unwrap();

        assert_eq!(
            svc.load_ephemeral_historic_checkpoint().await.unwrap(),
            None
        );
    }
}
