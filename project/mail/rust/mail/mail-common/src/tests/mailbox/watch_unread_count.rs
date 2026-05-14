use super::unread_count_watcher::resolve_unread;
use crate::datatypes::{MailSettingsId, ViewMode};
use crate::models::{ConversationCounter, MailSettings, MessageCounter};
use crate::test_utils::utils::test_address;
use crate::{conv_id, conversation, message, msg_id};
use mail_common::test_utils::db::new_test_connection;
use mail_core_common::datatypes::SystemLabel;
use mail_stash::orm::Model;

#[tokio::test]
async fn resolve_unread_conversations_returns_counter_value() {
    let mail_stash = new_test_connection().await;
    let mut tether = mail_stash.connection();
    let inbox = SystemLabel::Inbox.load(&tether).await.unwrap().unwrap();

    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            let mut counter = ConversationCounter::new(inbox.id());
            counter.unread = 5;
            counter.save(tx).await?;
            Ok(())
        })
        .await
        .unwrap();

    let count = resolve_unread(inbox.id(), ViewMode::Conversations, None, &tether)
        .await
        .unwrap();

    assert_eq!(count, 5);
}

#[tokio::test]
async fn resolve_unread_messages_returns_counter_value() {
    let mail_stash = new_test_connection().await;
    let mut tether = mail_stash.connection();
    let inbox = SystemLabel::Inbox.load(&tether).await.unwrap().unwrap();

    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            let mut counter = MessageCounter::new(inbox.id());
            counter.unread = 9;
            counter.save(tx).await?;
            Ok(())
        })
        .await
        .unwrap();

    let count = resolve_unread(inbox.id(), ViewMode::Messages, None, &tether)
        .await
        .unwrap();

    assert_eq!(count, 9);
}

#[tokio::test]
async fn resolve_unread_category_returns_one_when_has_unseen() {
    let mail_stash = new_test_connection().await;
    let mut tether = mail_stash.connection();

    let inbox = SystemLabel::Inbox.load(&tether).await.unwrap().unwrap();
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    let default = SystemLabel::CategoryDefault
        .load(&tether)
        .await
        .unwrap()
        .unwrap();

    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            MailSettings {
                local_id: MailSettingsId,
                mail_category_view: true,
                ..Default::default()
            }
            .save(tx)
            .await?;

            social.display = true;
            social.save(tx).await?;

            MessageCounter::new(inbox.id()).save(tx).await?;
            ConversationCounter::new(inbox.id()).save(tx).await?;
            MessageCounter::new(default.id()).save(tx).await?;
            ConversationCounter::new(default.id()).save(tx).await?;

            let mut msg_counter = MessageCounter::new(social.id());
            msg_counter.unread = 1;
            msg_counter.save(tx).await?;
            let mut conv_counter = ConversationCounter::new(social.id());
            conv_counter.unread = 1;
            conv_counter.save(tx).await?;

            let mut conv = conversation!(remote_id: conv_id!("test_conv"));
            conv.save(tx).await?;
            let mut address = test_address();
            address.save(tx).await?;
            let mut msg = message!(
                remote_id: msg_id!(1_u64),
                display_order: 1_u64,
                time: 1_u64.into()
            );
            msg.local_address_id = address.id();
            msg.remote_address_id = address.remote_id.clone().unwrap();
            msg.local_conversation_id = conv.local_id;
            msg.remote_conversation_id = conv.remote_id.clone();
            msg.label_ids = vec![
                inbox.remote_id.clone().unwrap(),
                social.remote_id.clone().unwrap(),
            ];
            msg.unread = true;
            msg.save(tx).await?;
            Ok(())
        })
        .await
        .unwrap();

    let count = resolve_unread(inbox.id(), ViewMode::Messages, Some(social.id()), &tether)
        .await
        .unwrap();

    assert_eq!(count, 1);
}

#[tokio::test]
async fn resolve_unread_category_returns_zero_when_no_unseen() {
    let mail_stash = new_test_connection().await;
    let mut tether = mail_stash.connection();
    let inbox = SystemLabel::Inbox.load(&tether).await.unwrap().unwrap();
    let social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    let default = SystemLabel::CategoryDefault
        .load(&tether)
        .await
        .unwrap()
        .unwrap();

    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            MailSettings {
                local_id: MailSettingsId,
                mail_category_view: true,
                ..Default::default()
            }
            .save(tx)
            .await?;
            MessageCounter::new(inbox.id()).save(tx).await?;
            ConversationCounter::new(inbox.id()).save(tx).await?;
            MessageCounter::new(default.id()).save(tx).await?;
            ConversationCounter::new(default.id()).save(tx).await?;
            MessageCounter::new(social.id()).save(tx).await?;
            ConversationCounter::new(social.id()).save(tx).await?;
            Ok(())
        })
        .await
        .unwrap();

    let count = resolve_unread(inbox.id(), ViewMode::Messages, Some(social.id()), &tether)
        .await
        .unwrap();

    assert_eq!(count, 0);
}

#[tokio::test]
async fn resolve_unread_default_includes_deactivated_category_unread() {
    let mail_stash = new_test_connection().await;
    let mut tether = mail_stash.connection();
    let inbox = SystemLabel::Inbox.load(&tether).await.unwrap().unwrap();
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    let default = SystemLabel::CategoryDefault
        .load(&tether)
        .await
        .unwrap()
        .unwrap();

    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            MailSettings {
                local_id: MailSettingsId,
                mail_category_view: true,
                ..Default::default()
            }
            .save(tx)
            .await?;

            social.display = true;
            social.save(tx).await?;

            let mut inbox_conv = ConversationCounter::new(inbox.id());
            inbox_conv.unread = 5;
            inbox_conv.save(tx).await?;
            MessageCounter::new(inbox.id()).save(tx).await?;
            let mut default_conv = ConversationCounter::new(default.id());
            default_conv.unread = 1;
            default_conv.save(tx).await?;
            MessageCounter::new(default.id()).save(tx).await?;

            let mut social_conv = ConversationCounter::new(social.id());
            social_conv.unread = 3;
            social_conv.save(tx).await?;
            MessageCounter::new(social.id()).save(tx).await?;

            Ok(())
        })
        .await
        .unwrap();

    // Social is available — its unread stays separate; CategoryDefault has 0
    let before_def = resolve_unread(
        inbox.id(),
        ViewMode::Conversations,
        Some(default.id()),
        &tether,
    )
    .await
    .unwrap();
    assert_eq!(before_def, 1);

    let before_soc = resolve_unread(
        inbox.id(),
        ViewMode::Conversations,
        Some(social.id()),
        &tether,
    )
    .await
    .unwrap();
    assert_eq!(before_soc, 3);

    // Deactivate social
    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            social.display = false;
            social.save(tx).await?;
            Ok(())
        })
        .await
        .unwrap();

    // Social is no longer available — its unread folds into CategoryDefault
    let after_def = resolve_unread(
        inbox.id(),
        ViewMode::Conversations,
        Some(default.id()),
        &tether,
    )
    .await
    .unwrap();
    assert_eq!(after_def, 4);
    // So if disabled it should return the unread of default
    let after_soc = resolve_unread(
        inbox.id(),
        ViewMode::Conversations,
        Some(social.id()),
        &tether,
    )
    .await
    .unwrap();
    assert_eq!(after_soc, 4);

    // Deactivate category_view
    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            MailSettings {
                local_id: MailSettingsId,
                mail_category_view: false,
                ..Default::default()
            }
            .save(tx)
            .await?;
            Ok(())
        })
        .await
        .unwrap();

    // CategoryView is no longer available - default back to inbox
    let after_def = resolve_unread(
        inbox.id(),
        ViewMode::Conversations,
        Some(default.id()),
        &tether,
    )
    .await
    .unwrap();
    assert_eq!(after_def, 5);
    let after_soc = resolve_unread(
        inbox.id(),
        ViewMode::Conversations,
        Some(social.id()),
        &tether,
    )
    .await
    .unwrap();
    assert_eq!(after_soc, 5);
}
