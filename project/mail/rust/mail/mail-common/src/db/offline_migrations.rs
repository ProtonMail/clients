mod v001_proton_mail_default_labels;
mod v005_proton_mail_conversation_counters;
mod v007_proton_mail_message_counters;
mod v016_proton_mail_new_system_labels;
mod v019_proton_mail_draft_send_result_refactor;
mod v045_proton_mail_draft_send_result;
mod v046_proton_mail_android_signatures;
mod v061_proton_mail_restore_non_expired_messages;

use include_dir::{Dir, include_dir};
use mail_sqlite3::file::embedded_migrations;
use mail_sqlite3::{Migrator, MigratorError};
use mail_stash::UserDb;
use mail_stash::stash::Stash;

pub async fn run(mail_stash: &Stash<UserDb>) -> Result<usize, MigratorError> {
    const TABLE: &str = "proton_mail_db_version";
    const MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/db/offline_migrations");

    let mut migrations = embedded_migrations(&MIGRATIONS);

    migrations.push(Box::new(
        v001_proton_mail_default_labels::DefaultLabelsMigration,
    ));
    migrations.push(Box::new(
        v005_proton_mail_conversation_counters::ConversationCountersMigration,
    ));
    migrations.push(Box::new(
        v007_proton_mail_message_counters::MessageCountersMigration,
    ));
    migrations.push(Box::new(
        v016_proton_mail_new_system_labels::DefaultLabelsMigration,
    ));
    migrations.push(Box::new(
        v019_proton_mail_draft_send_result_refactor::DraftSendResultMigration,
    ));
    migrations.push(Box::new(
        v045_proton_mail_draft_send_result::DraftSendResultAttachmentErrorsMigration,
    ));
    migrations.push(Box::new(
        v046_proton_mail_android_signatures::AndroidSignaturesMigration,
    ));
    migrations.push(Box::new(
        v061_proton_mail_restore_non_expired_messages::RestoreNonExpiredMessages,
    ));
    let mut tether = mail_stash.connection();

    Migrator::new(TABLE, migrations).migrate(&mut tether).await
}

#[cfg(test)]
mod tests {
    use super::{Stash, run as migrate_mail_db};
    use mail_core_common::db::migrations::migrate_core_db;

    #[tokio::test]
    async fn smoke() {
        let mail_stash = Stash::new(None).unwrap();

        migrate_core_db(&mail_stash).await.unwrap();
        migrate_mail_db(&mail_stash).await.unwrap();
    }
}
