use indoc::indoc;
use mail_api::services::proton::common::MessageId;
use mail_core_common::datatypes::UnixTimestamp;
use mail_core_common::declare_local_id;
use mail_stash::macros::{DbRecord, Model};
use mail_stash::orm::Model;
use mail_stash::stash::{StashError, Tether, WriteTx};
use mail_stash::{UserDb, params, rusqlite};

declare_local_id!(SyncBatchId);

#[derive(Debug, DbRecord, Clone, Eq, PartialEq, Hash)]
pub struct SyncBatch {
    #[DbField]
    pub id: SyncBatchId,
    #[DbField]
    pub begin_id: MessageId,
    #[DbField]
    pub begin_time: UnixTimestamp,
    #[DbField]
    pub end_id: MessageId,
    #[DbField]
    pub end_time: UnixTimestamp,
    #[DbField]
    pub sync_time: UnixTimestamp,
}

impl SyncBatch {
    pub async fn create(
        begin_id: MessageId,
        begin_time: UnixTimestamp,
        end_id: MessageId,
        end_time: UnixTimestamp,
        tx: &WriteTx<'_>,
    ) -> Result<Self, StashError> {
        tx.sync_bridge(move |tx| {
            let sync_time = UnixTimestamp::now();
            let id: SyncBatchId = tx.query_one(
                indoc! {
                    "INSERT INTO mail_sync_batch (
                    begin_id,
                    begin_time,
                    end_id,
                    end_time,
                    sync_time
                ) VALUES (?,?,?,?,?)
                RETURNING id
                "
                },
                rusqlite::params![begin_id, begin_time, end_id, end_time, sync_time],
                |r| r.get(0),
            )?;
            Ok(Self {
                id,
                begin_id,
                begin_time,
                end_id,
                end_time,
                sync_time,
            })
        })
        .await
    }

    pub async fn find_by_id(id: SyncBatchId, tether: &Tether) -> Result<Option<Self>, StashError> {
        Ok(tether
            .query::<_, Self>(
                "SELECT * FROM mail_sync_batch WHERE id =? LIMIT 1",
                params![id],
            )
            .await?
            .pop())
    }

    pub async fn find_oldest_batch(tether: &Tether) -> Result<Option<Self>, StashError> {
        Ok(tether
            .query::<_, Self>(
                "SELECT * FROM mail_sync_batch ORDER BY end_time ASC LIMIT 1",
                params![],
            )
            .await?
            .pop())
    }

    pub async fn find_newest_batch(tether: &Tether) -> Result<Option<Self>, StashError> {
        Ok(tether
            .query::<_, Self>(
                "SELECT * FROM mail_sync_batch ORDER BY begin_time DESC LIMIT 1",
                params![],
            )
            .await?
            .pop())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Model)]
#[Database(UserDb)]
#[TableName("mail_sync_settings")]
pub struct SyncSettings {
    #[IdField]
    id: u64,
    #[DbField]
    pub backward_sync_start: Option<UnixTimestamp>,
    #[DbField]
    pub backward_sync_complete: Option<UnixTimestamp>,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            id: SYNC_SETTINGS_ID,
            backward_sync_start: None,
            backward_sync_complete: None,
        }
    }
}

const SYNC_SETTINGS_ID: u64 = 1;

impl SyncSettings {
    pub async fn get(tether: &Tether) -> Result<Option<Self>, StashError> {
        Self::load(SYNC_SETTINGS_ID, tether).await
    }

    pub async fn get_or_create(tx: &WriteTx<'_>) -> Result<Self, StashError> {
        match Self::load(SYNC_SETTINGS_ID, tx).await {
            Ok(Some(v)) => Ok(v),
            Ok(None) => {
                let mut setting = Self::default();
                setting.save(tx).await?;
                Ok(setting)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn mark_backward_sync_start(tx: &WriteTx<'_>) -> Result<(), StashError> {
        tx.execute(
            "UPDATE mail_sync_settings SET backward_sync_start = ? WHERE backward_sync_start IS NULL AND id = ?",
            params![Some(UnixTimestamp::now()), SYNC_SETTINGS_ID],
        )
        .await?;
        Ok(())
    }

    pub async fn mark_backward_sync_complete(tx: &WriteTx<'_>) -> Result<(), StashError> {
        tx.execute(
            "UPDATE mail_sync_settings SET backward_sync_complete = ? WHERE id =? ",
            params![Some(UnixTimestamp::now()), SYNC_SETTINGS_ID],
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_common::test_utils::db::new_test_connection_file;

    fn msg_id(s: &str) -> MessageId {
        MessageId::from(s.to_string())
    }

    async fn create_batch(
        tether: &mut Tether,
        begin_id: &str,
        begin_time: u64,
        end_id: &str,
        end_time: u64,
    ) -> SyncBatch {
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                SyncBatch::create(
                    msg_id(begin_id),
                    UnixTimestamp::new(begin_time),
                    msg_id(end_id),
                    UnixTimestamp::new(end_time),
                    tx,
                )
                .await
            })
            .await
            .unwrap()
    }

    // -- SyncBatch ----------------------------------------------------------

    #[tokio::test]
    async fn create_persists_all_fields() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let mut tether = stash.connection();

        let before = UnixTimestamp::now();
        let batch = create_batch(&mut tether, "begin-id", 1_000, "end-id", 500).await;
        let after = UnixTimestamp::now();

        let loaded = SyncBatch::find_by_id(batch.id, &tether)
            .await
            .unwrap()
            .expect("batch should be persisted");

        assert_eq!(loaded.id, batch.id);
        assert_eq!(loaded.begin_id, msg_id("begin-id"));
        assert_eq!(loaded.begin_time, UnixTimestamp::new(1_000));
        assert_eq!(loaded.end_id, msg_id("end-id"));
        assert_eq!(loaded.end_time, UnixTimestamp::new(500));
        assert!(loaded.sync_time >= before && loaded.sync_time <= after);
    }

    #[tokio::test]
    async fn find_oldest_batch_returns_none_when_empty() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let tether = stash.connection();

        let result = SyncBatch::find_oldest_batch(&tether).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn find_oldest_batch_picks_batch_with_smallest_end_time() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let mut tether = stash.connection();

        // Insert out-of-order so we don't accidentally rely on row order.
        create_batch(&mut tether, "b-new", 300, "e-new", 250).await;
        create_batch(&mut tether, "b-mid", 200, "e-mid", 150).await;
        let oldest = create_batch(&mut tether, "b-old", 100, "e-old", 50).await;

        let result = SyncBatch::find_oldest_batch(&tether)
            .await
            .unwrap()
            .expect("should find a batch");

        assert_eq!(result.id, oldest.id);
        assert_eq!(result.end_time, UnixTimestamp::new(50));
    }

    #[tokio::test]
    async fn find_newest_batch_returns_none_when_empty() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let tether = stash.connection();

        let result = SyncBatch::find_newest_batch(&tether).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn find_newest_batch_picks_batch_with_largest_begin_time() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let mut tether = stash.connection();

        let newest = create_batch(&mut tether, "b-new", 300, "e-new", 250).await;
        create_batch(&mut tether, "b-mid", 200, "e-mid", 150).await;
        create_batch(&mut tether, "b-old", 100, "e-old", 50).await;

        let result = SyncBatch::find_newest_batch(&tether)
            .await
            .unwrap()
            .expect("should find a batch");

        assert_eq!(result.id, newest.id);
        assert_eq!(result.begin_time, UnixTimestamp::new(300));
    }

    // -- SyncSettings -------------------------------------------------------

    #[tokio::test]
    async fn settings_get_returns_none_on_fresh_db() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let tether = stash.connection();

        let result = SyncSettings::get(&tether).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn settings_get_or_create_inserts_then_returns_same_row() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let mut tether = stash.connection();

        let first = tether
            .write_tx::<_, _, StashError>(async |tx| SyncSettings::get_or_create(tx).await)
            .await
            .unwrap();
        assert_eq!(first, SyncSettings::default());

        // Second call must return the persisted row, not a fresh default.
        let second = tether
            .write_tx::<_, _, StashError>(async |tx| {
                SyncSettings::mark_backward_sync_start(tx).await?;
                SyncSettings::get_or_create(tx).await
            })
            .await
            .unwrap();
        assert_ne!(first, second);

        // And get() should now see it.
        let loaded = SyncSettings::get(&tether).await.unwrap();
        assert_eq!(loaded.as_ref(), Some(&second));
    }

    /// `mark_backward_sync_start` is gated by `backward_sync_start IS NULL`,
    /// so calling it twice must leave the first timestamp untouched.
    #[tokio::test]
    async fn settings_mark_backward_sync_start_is_idempotent() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let mut tether = stash.connection();

        tether
            .write_tx::<_, _, StashError>(async |tx| {
                SyncSettings::get_or_create(tx).await?;
                SyncSettings::mark_backward_sync_start(tx).await
            })
            .await
            .unwrap();

        let first = SyncSettings::get(&tether)
            .await
            .unwrap()
            .unwrap()
            .backward_sync_start
            .expect("first mark should set the timestamp");

        // Sleep so a non-idempotent UPDATE would produce a strictly later value.
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;

        tether
            .write_tx::<_, _, StashError>(async |tx| {
                SyncSettings::mark_backward_sync_start(tx).await
            })
            .await
            .unwrap();

        let second = SyncSettings::get(&tether)
            .await
            .unwrap()
            .unwrap()
            .backward_sync_start
            .unwrap();

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn settings_mark_backward_sync_complete_sets_timestamp() {
        let (stash, _db_dir) = new_test_connection_file().await;
        let mut tether = stash.connection();

        tether
            .write_tx::<_, _, StashError>(async |tx| {
                SyncSettings::get_or_create(tx).await?;
                SyncSettings::mark_backward_sync_complete(tx).await
            })
            .await
            .unwrap();

        SyncSettings::get(&tether)
            .await
            .unwrap()
            .unwrap()
            .backward_sync_complete
            .expect("complete timestamp should be set");
    }
}
