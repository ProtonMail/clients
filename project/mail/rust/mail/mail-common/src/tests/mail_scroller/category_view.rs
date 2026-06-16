use crate::datatypes::MailSettingsId;
use crate::mail_scroller::category_view::CategoryView;
use crate::models::{ConversationCounter, MailSettings, MessageCounter};
use crate::test_utils::feature_flags::enable_category_view_ff;
use crate::test_utils::test_context::MailTestContext;
use crate::test_utils::utils::test_address;
use crate::{self as mail_common, MailContextError};
use mail_common::{conv_id, conversation, message, msg_id};
use mail_core_api::services::proton::EventId;
use mail_core_common::datatypes::SystemLabel;
use mail_core_common::models::Label;
use mail_stash::orm::Model;
use mail_stash::stash::{StashError, WriteTx};

async fn enable_category_view_setting(bond: &WriteTx<'_>) {
    MailSettings {
        local_id: MailSettingsId,
        mail_category_view: true,
        ..Default::default()
    }
    .save(bond)
    .await
    .unwrap();
    enable_category_view_ff(bond).await.unwrap();
}

async fn store_single_unseen_in_category(category: &Label, bond: &WriteTx<'_>) {
    // Inbox and default should be available always
    let inbox = SystemLabel::Inbox.load(bond).await.unwrap().unwrap();
    MessageCounter::new(inbox.id()).save(bond).await.unwrap();
    ConversationCounter::new(inbox.id())
        .save(bond)
        .await
        .unwrap();
    let default = SystemLabel::CategoryDefault
        .load(bond)
        .await
        .unwrap()
        .unwrap();
    MessageCounter::new(default.id()).save(bond).await.unwrap();
    ConversationCounter::new(default.id())
        .save(bond)
        .await
        .unwrap();

    // MessageCounter and ConversationCounter with unread=1 to reflect the unread message
    // in the chosen category (one unread message implies one unread conversation).
    let mut conv = conversation!(remote_id: conv_id!("unseen_conv"));
    conv.save(bond).await.unwrap();
    let mut category_msg_counter = MessageCounter::new(category.id());
    category_msg_counter.unread = 1;
    category_msg_counter.total = 1;
    category_msg_counter.save(bond).await.unwrap();
    let mut category_conv_counter = ConversationCounter::new(category.id());
    category_conv_counter.unread = 1;
    category_conv_counter.total = 1;
    category_conv_counter.save(bond).await.unwrap();

    // One unread message in chosen category
    let mut address = test_address();
    address.save(bond).await.unwrap();
    let mut msg =
        message!(remote_id: msg_id!(100_u64), display_order: 100_u64, time: 100_u64.into());
    msg.local_address_id = address.id();
    msg.remote_address_id = address.remote_id.clone().unwrap();
    msg.local_conversation_id = conv.local_id;
    msg.remote_conversation_id = conv.remote_id.clone();
    msg.label_ids = vec![
        inbox.remote_id.clone().unwrap(),
        category.remote_id.clone().unwrap(),
    ];
    msg.unread = true;
    msg.save(bond).await.unwrap();

    // The unseen badge is driven by the label's own last_unseen_message, which the
    // backend stamps when the category holds an unseen message.
    let mut category = category.clone();
    category.last_unseen_message = Some(EventId::from("unseen_event"));
    category.save(bond).await.unwrap();
}

#[tokio::test]
async fn test_category_label_has_unseen_items_from_db() {
    let test_ctx = MailTestContext::new().await;
    let ctx = test_ctx.uninitialized_mail_user_context().await;
    let mut tether = ctx.user_stash().connection();
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            enable_category_view_setting(bond).await;
            social.display = true;
            social.save(bond).await?;
            store_single_unseen_in_category(&social, bond).await;
            Ok(())
        })
        .await
        .unwrap();

    let inbox_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let view = CategoryView::load(inbox_id, &ctx).await.unwrap();
    let labels = view.into_labels(&tether).await.unwrap();
    let social_label = labels
        .iter()
        .find(|l| l.system_label == SystemLabel::CategorySocial)
        .unwrap();
    assert!(
        social_label.has_unseen_items,
        "should have unseen items with unread message"
    );
}

#[tokio::test]
async fn test_expanded_filter_ids_category_default() {
    let test_ctx = MailTestContext::new().await;
    let ctx = test_ctx.uninitialized_mail_user_context().await;
    let mut tether = ctx.user_stash().connection();
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    let mut updates = SystemLabel::CategoryUpdates
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
        .write_tx::<_, _, StashError>(async |bond| {
            enable_category_view_setting(bond).await;
            social.display = false;
            social.save(bond).await.unwrap();
            updates.display = true;
            updates.save(bond).await.unwrap();

            Ok(())
        })
        .await
        .unwrap();

    let default_id = default.id();
    let social_id = social.id();
    let updates_id = updates.id();

    let inbox_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let mut view = CategoryView::load(inbox_id, &ctx).await.unwrap();
    view.enable(Some(default_id), &tether).await.unwrap();

    assert!(
        view.filter_ids.contains(&default_id),
        "CategoryDefault itself must be in expanded filter"
    );
    assert!(
        view.filter_ids.contains(&social_id),
        "display=false CategorySocial must be in CategoryDefault expansion"
    );
    assert!(
        !view.filter_ids.contains(&updates_id),
        "CategoryUpdates cannot be in CategoryDefault expansion"
    );

    let err = view.enable(Some(social_id), &tether).await.unwrap_err();
    assert!(
        matches!(err, MailContextError::CategoryNotSupported),
        "display=false CategorySocial cannot be enabled for a non-default category"
    );

    view.enable(Some(updates_id), &tether).await.unwrap();

    assert!(
        !view.filter_ids.contains(&default_id),
        "CategoryDefault cannot be in expanded filter for non-default category"
    );
    assert!(
        !view.filter_ids.contains(&social_id),
        "display=false CategorySocial cannot be in expanded filter for non-default category"
    );
    assert!(
        view.filter_ids.contains(&updates_id),
        "CategoryUpdates is enabled category and should only be returned"
    );
}

#[tokio::test]
async fn test_load_returns_empty_when_setting_disabled() {
    let test_ctx = MailTestContext::new().await;
    let ctx = test_ctx.uninitialized_mail_user_context().await;
    let tether = ctx.user_stash().connection();
    // No MailSettings stored → mail_category_view defaults to false.
    let inbox_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let view = CategoryView::load(inbox_id, &ctx).await.unwrap();
    assert!(
        view.available.is_empty(),
        "available should be empty when mail_category_view is disabled"
    );
    assert!(
        view.enabled.is_none(),
        "enabled should be None when mail_category_view is disabled"
    );
}

// ── MailSettings → CategoryViewChanged reactive tests ────────────────────────

#[tokio::test]
async fn test_settings_toggle_enabled_to_disabled_produces_empty_view() {
    let test_ctx = MailTestContext::new().await;
    let ctx = test_ctx.uninitialized_mail_user_context().await;
    let mut tether = ctx.user_stash().connection();

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            enable_category_view_setting(bond).await;
            Ok(())
        })
        .await
        .unwrap();

    let inbox_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let view_enabled = CategoryView::load(inbox_id, &ctx).await.unwrap();
    assert!(
        !view_enabled.available.is_empty(),
        "precondition: feature on"
    );

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                local_id: MailSettingsId,
                mail_category_view: false,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            Ok(())
        })
        .await
        .unwrap();

    let view_disabled = CategoryView::load(inbox_id, &ctx).await.unwrap();
    assert!(
        view_disabled.available.is_empty(),
        "available must be empty after feature is toggled off"
    );
    assert!(
        view_disabled.enabled.is_none(),
        "enabled must be None after feature is toggled off"
    );
}

#[tokio::test]
async fn test_settings_toggle_disabled_to_enabled_populates_view() {
    let test_ctx = MailTestContext::new().await;
    let ctx = test_ctx.uninitialized_mail_user_context().await;
    let mut tether = ctx.user_stash().connection();

    // Start with feature disabled (no MailSettings row → defaults to false).
    let inbox_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let view_before = CategoryView::load(inbox_id, &ctx).await.unwrap();
    assert!(
        view_before.available.is_empty(),
        "precondition: feature off"
    );

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            enable_category_view_setting(bond).await;
            Ok(())
        })
        .await
        .unwrap();

    let view_after = CategoryView::load(inbox_id, &ctx).await.unwrap();
    assert!(
        !view_after.available.is_empty(),
        "available must be non-empty after feature is toggled on"
    );
    assert!(
        view_after.enabled.is_some(),
        "enabled should default to CategoryDefault after feature is toggled on"
    );
}

#[tokio::test]
async fn test_unrelated_settings_change_does_not_alter_category_view() {
    let test_ctx = MailTestContext::new().await;
    let ctx = test_ctx.uninitialized_mail_user_context().await;
    let mut tether = ctx.user_stash().connection();

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            enable_category_view_setting(bond).await;
            Ok(())
        })
        .await
        .unwrap();

    let inbox_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let view_before = CategoryView::load(inbox_id, &ctx).await.unwrap();

    // Change an unrelated MailSettings field.
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                local_id: MailSettingsId,
                mail_category_view: true,
                auto_save_contacts: true,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            Ok(())
        })
        .await
        .unwrap();

    let view_after = CategoryView::load(inbox_id, &ctx).await.unwrap();
    assert_eq!(
        view_before.available, view_after.available,
        "available must not change when an unrelated setting changes"
    );
    assert_eq!(
        view_before.enabled.is_some(),
        view_after.enabled.is_some(),
        "enabled presence must not change when an unrelated setting changes"
    );
}

#[tokio::test]
async fn test_settings_change_noop_for_non_inbox_label() {
    let test_ctx = MailTestContext::new().await;
    let ctx = test_ctx.uninitialized_mail_user_context().await;
    let mut tether = ctx.user_stash().connection();

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            enable_category_view_setting(bond).await;
            Ok(())
        })
        .await
        .unwrap();

    // Sent is not Inbox → CategoryView::load must return default regardless.
    let sent_id = SystemLabel::Sent.local_id(&tether).await.unwrap().unwrap();
    let view = CategoryView::load(sent_id, &ctx).await.unwrap();
    assert!(
        view.available.is_empty(),
        "category view must be empty for non-Inbox labels even when setting is enabled"
    );
}
