mod contact;
mod contact_card;
mod contact_email;
mod contact_group;

use mail_db::Database;
use mail_db_stash::{ReadTx, StashDb, WriteTx};
use mail_stash::UserDb;
use mail_stash::stash::StashError;

pub struct ContactReadTx<'tx>(ReadTx<'tx, UserDb>);

impl mail_db::Transaction for ContactReadTx<'_> {
    type Error = StashError;
}

impl mail_db::ReadTx for ContactReadTx<'_> {}

pub struct ContactWriteTx<'tx>(WriteTx<'tx, UserDb>);

impl mail_db::Transaction for ContactWriteTx<'_> {
    type Error = StashError;
}

impl mail_db::ReadTx for ContactWriteTx<'_> {}
impl mail_db::WriteTx for ContactWriteTx<'_> {}

pub struct ContactStashDb(StashDb<UserDb>);

impl Database for ContactStashDb {
    type Error = <StashDb<UserDb> as Database>::Error;

    type ReadTx<'a> = ContactReadTx<'a>;

    type WriteTx<'a> = ContactWriteTx<'a>;

    async fn read<T, E: From<Self::Error>>(
        &self,
        closure: impl AsyncFnOnce(Self::ReadTx<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        self.0
            .read(async move |tx| {
                let tx = ContactReadTx(tx);
                closure(tx).await
            })
            .await
    }

    async fn write<T, E: From<Self::Error>>(
        &self,
        closure: impl AsyncFnOnce(Self::WriteTx<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        self.0
            .write(async move |tx| {
                let tx = ContactWriteTx(tx);
                closure(tx).await
            })
            .await
    }
}
