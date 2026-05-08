use super::{UnreadCountWatcher, UnreadWatchScope};
use crate::datatypes::MailSettingsId;
use crate::models::{ConversationCounter, MailSettings, MessageCounter};
use mail_common::test_utils::db::new_test_connection;
use mail_core_common::datatypes::SystemLabel;
use mail_core_common::models::Label;
use mail_stash::orm::Model;
use tokio::time::{Duration, timeout};

#[test]
fn tables_category_returns_all_four() {
    let tables = UnreadWatchScope::Category.tables();
    assert_eq!(
        tables,
        vec![
            ConversationCounter::table_name(),
            MessageCounter::table_name(),
            MailSettings::table_name(),
            Label::table_name(),
        ]
    );
}

#[test]
fn tables_conversations_returns_only_conversation_counter() {
    assert_eq!(
        UnreadWatchScope::Conversations.tables(),
        vec![ConversationCounter::table_name()]
    );
}

#[test]
fn tables_messages_returns_only_message_counter() {
    assert_eq!(
        UnreadWatchScope::Messages.tables(),
        vec![MessageCounter::table_name()]
    );
}

#[tokio::test]
async fn watch_category_fires_on_conversation_counter_change() {
    let mail_stash = new_test_connection().await;
    let handle = UnreadCountWatcher::watch(UnreadWatchScope::Category, &mail_stash)
        .await
        .unwrap();

    let tether = mail_stash.connection().await.unwrap();
    let inbox = SystemLabel::Inbox.load(&tether).await.unwrap().unwrap();
    drop(tether);

    let mut tether = mail_stash.connection().await.unwrap();
    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            ConversationCounter::new(inbox.id()).save(tx).await?;
            Ok(())
        })
        .await
        .unwrap();

    timeout(Duration::from_secs(5), handle.receiver.recv_async())
        .await
        .expect("timed out waiting for notification")
        .expect("channel closed");
}

#[tokio::test]
async fn watch_category_fires_on_mail_settings_change() {
    let mail_stash = new_test_connection().await;
    let handle = UnreadCountWatcher::watch(UnreadWatchScope::Category, &mail_stash)
        .await
        .unwrap();

    let mut tether = mail_stash.connection().await.unwrap();
    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            MailSettings {
                local_id: MailSettingsId,
                mail_category_view: true,
                ..Default::default()
            }
            .save(tx)
            .await?;
            Ok(())
        })
        .await
        .unwrap();

    timeout(Duration::from_secs(5), handle.receiver.recv_async())
        .await
        .expect("timed out waiting for notification on MailSettings change")
        .expect("channel closed");
}

#[tokio::test]
async fn watch_category_fires_on_label_display_change() {
    let mail_stash = new_test_connection().await;
    let handle = UnreadCountWatcher::watch(UnreadWatchScope::Category, &mail_stash)
        .await
        .unwrap();

    let tether = mail_stash.connection().await.unwrap();
    let mut label = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    drop(tether);

    let mut tether = mail_stash.connection().await.unwrap();
    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            label.display = !label.display;
            label.save(tx).await?;
            Ok(())
        })
        .await
        .unwrap();

    timeout(Duration::from_secs(5), handle.receiver.recv_async())
        .await
        .expect("timed out waiting for notification on Label change")
        .expect("channel closed");
}

#[tokio::test]
async fn watch_conversations_does_not_fire_on_mail_settings_change() {
    let mail_stash = new_test_connection().await;
    let handle = UnreadCountWatcher::watch(UnreadWatchScope::Conversations, &mail_stash)
        .await
        .unwrap();

    let mut tether = mail_stash.connection().await.unwrap();
    tether
        .write_tx::<_, _, anyhow::Error>(async |tx| {
            MailSettings {
                local_id: MailSettingsId,
                mail_category_view: true,
                ..Default::default()
            }
            .save(tx)
            .await?;
            Ok(())
        })
        .await
        .unwrap();

    let result = timeout(Duration::from_secs(2), handle.receiver.recv_async()).await;
    assert!(
        result.is_err(),
        "expected no notification for unrelated table change"
    );
}
