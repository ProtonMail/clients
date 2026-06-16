use crate::models::MailboxLabels;
use mail_common::test_utils::db::new_test_connection;
use mail_core_common::datatypes::{LabelColor, LabelType};
use mail_core_common::models::Label;
use mail_stash::orm::Model;
use mail_stash::stash::StashError;

#[tokio::test]
async fn test_mark_labels_as_initialized() {
    let mut tether = new_test_connection().await.connection();
    tether
        .write_tx::<_, _, StashError>(async |tx| {
            let mut new_label = Label {
                remote_id: Some("MyLabel".into()),
                color: LabelColor::purple(),
                label_type: LabelType::Folder,
                name: "Label".to_owned(),
                ..Label::test_default()
            };
            new_label.save(tx).await.expect("failed to create label");
            let new_label_id = new_label.id();

            let mut mailbox_label = MailboxLabels::load(new_label_id, tx)
                .await
                .expect("failed to load label")
                .unwrap_or_else(|| MailboxLabels::new(new_label_id));

            // Newly created label is not initialized
            assert!(!mailbox_label.initialized);

            // Initializing
            mailbox_label.initialized = true;
            mailbox_label
                .save(tx)
                .await
                .expect("failed to mark label as initialized");

            // Load from the DB again
            let mailbox_label = MailboxLabels::load(new_label_id, tx)
                .await
                .expect("failed to load label")
                .unwrap_or_else(|| MailboxLabels::new(new_label_id));

            // Now it should be marked as initialized
            assert!(mailbox_label.initialized);
            Ok(())
        })
        .await
        .unwrap();
}
