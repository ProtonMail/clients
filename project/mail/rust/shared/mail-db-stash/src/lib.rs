use std::ops::Deref;

use mail_db::{Database, Transaction};
use mail_stash::{
    marker::DatabaseMarker,
    stash::{ReadTx as StashReadTx, Stash, StashError, Tether, WriteTx as StashWriteTx},
};

pub struct ReadTx<'t, M: DatabaseMarker>(&'t StashReadTx<'t, M>);

impl<M: DatabaseMarker> Deref for ReadTx<'_, M> {
    type Target = Tether<M>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<M: DatabaseMarker> Transaction for ReadTx<'_, M> {
    type Error = StashError;
}

impl<M: DatabaseMarker> mail_db::ReadTx for ReadTx<'_, M> {}

pub struct WriteTx<'t, M: DatabaseMarker>(&'t StashWriteTx<'t, M>);

impl<'t, M: DatabaseMarker> Deref for WriteTx<'t, M> {
    type Target = StashWriteTx<'t, M>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<M: DatabaseMarker> Transaction for WriteTx<'_, M> {
    type Error = StashError;
}

impl<M: DatabaseMarker> mail_db::ReadTx for WriteTx<'_, M> {}

impl<M: DatabaseMarker> mail_db::WriteTx for WriteTx<'_, M> {}

#[derive(Clone)]
pub struct StashDb<M: DatabaseMarker> {
    stash: Stash<M>,
}

impl<M: DatabaseMarker> StashDb<M> {
    pub fn new(stash: Stash<M>) -> Self {
        Self { stash }
    }

    pub fn instance(&self) -> &Stash<M> {
        &self.stash
    }
}

impl<M: DatabaseMarker> Database for StashDb<M> {
    type Error = StashError;

    type ReadTx<'a> = ReadTx<'a, M>;

    type WriteTx<'a> = WriteTx<'a, M>;

    async fn read<T, E: From<Self::Error>>(
        &self,
        closure: impl AsyncFnOnce(Self::ReadTx<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut tether = self.stash.connection();
        tether
            .read_tx(async |stash_rtx| {
                let rtx = ReadTx(stash_rtx);
                closure(rtx).await
            })
            .await
    }

    async fn write<T, E: From<Self::Error>>(
        &self,
        closure: impl AsyncFnOnce(Self::WriteTx<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut tether = self.stash.connection();
        tether
            .write_tx(async move |tx| {
                let tx = WriteTx(tx);
                closure(tx).await
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use mail_stash::{UserDb, params};

    use super::*;

    #[tokio::test]
    async fn read_write_tx() {
        let db_dir = tempfile::tempdir().unwrap();
        let stash: Stash<UserDb> =
            Stash::new(Some(&db_dir.path().join("test"))).expect("Failed to create Stash");
        let db = StashDb::new(stash);
        db.write(async |tx| {
            tx.execute("CREATE TABLE foo(bar INTEGER PRIMARY KEY)", params![])
                .await?;
            tx.execute("INSERT INTO foo VALUES (30)", params![]).await?;
            Ok::<_, StashError>(())
        })
        .await
        .unwrap();

        let value: u32 = db
            .read(async |tx| {
                tx.query_value("SELECT bar FROM foo LIMIT 1", params![])
                    .await
            })
            .await
            .unwrap();

        assert_eq!(value, 30);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn read_write_tx_spawned() {
        let db_dir = tempfile::tempdir().unwrap();
        let stash: Stash<UserDb> =
            Stash::new(Some(&db_dir.path().join("test"))).expect("Failed to create Stash");
        let db = StashDb::new(stash);
        let value: u32 = tokio::spawn(async move {
            db.write(async |tx| {
                tx.execute("CREATE TABLE foo(bar INTEGER PRIMARY KEY)", params![])
                    .await?;
                tx.execute("INSERT INTO foo VALUES (30)", params![]).await?;
                Ok::<_, StashError>(())
            })
            .await
            .unwrap();

            db.read(async |tx| {
                tx.query_value::<_, u32>("SELECT bar FROM foo LIMIT 1", params![])
                    .await
            })
            .await
            .unwrap()
        })
        .await
        .unwrap();

        assert_eq!(value, 30);
    }
}
