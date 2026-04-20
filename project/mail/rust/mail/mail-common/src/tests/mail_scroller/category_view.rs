use crate::datatypes::MailSettingsId;
use crate::mail_scroller::category_view::CategoryView;
use crate::models::{ConversationCounter, MailSettings, MessageCounter};
use crate::test_utils::utils::test_address;
use crate::{self as mail_common, MailContextError};
use mail_common::test_utils::db::new_test_connection;
use mail_common::{conv_id, conversation, message, msg_id};
use mail_core_common::datatypes::SystemLabel;
use mail_core_common::models::Label;
use mail_stash::orm::Model;
use mail_stash::stash::{Bond, StashError};

async fn enable_category_view_setting(bond: &Bond<'_>) {
    MailSettings {
        local_id: MailSettingsId,
        mail_category_view: true,
        ..Default::default()
    }
    .save(bond)
    .await
    .unwrap();
}

async fn store_single_unseen_in_category(category: &Label, bond: &Bond<'_>) {
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

    // MessageCounter with unread=1 to reflect the unread message in chosen category
    let mut conv = conversation!(remote_id: conv_id!("unseen_conv"));
    conv.save(bond).await.unwrap();
    let mut category_msg_counter = MessageCounter::new(category.id());
    category_msg_counter.unread = 1;
    category_msg_counter.total = 1;
    category_msg_counter.save(bond).await.unwrap();
    ConversationCounter::new(category.id())
        .save(bond)
        .await
        .unwrap();

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
}

#[tokio::test]
async fn test_category_label_has_unseen_items_from_db() {
    let mail_stash = new_test_connection().await;
    let mut tether = mail_stash.connection().await.unwrap();
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();

    tether
        .tx::<_, _, StashError>(async |bond| {
            enable_category_view_setting(bond).await;
            social.display = true;
            social.save(bond).await?;
            store_single_unseen_in_category(&social, bond).await;
            Ok(())
        })
        .await
        .unwrap();

    let view = CategoryView::load(&tether).await.unwrap();
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
async fn test_load_available_filters_display_and_carries_unseen_items_to_primary() {
    let mail_stash = new_test_connection().await;
    let mut tether = mail_stash.connection().await.unwrap();
    let default = SystemLabel::CategoryDefault
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    let mut promotions = SystemLabel::CategoryPromotions
        .load(&tether)
        .await
        .unwrap()
        .unwrap();

    tether
        .tx::<_, _, StashError>(async |bond| {
            enable_category_view_setting(bond).await;
            MessageCounter::new(social.id()).save(bond).await?;
            ConversationCounter::new(social.id())
                .save(bond)
                .await
                .unwrap();
            social.display = true;
            social.save(bond).await?;
            promotions.display = false;
            promotions.save(bond).await?;
            store_single_unseen_in_category(&promotions, bond).await;
            Ok(())
        })
        .await
        .unwrap();

    let view = CategoryView::load(&tether).await.unwrap();
    let social_id = social.id();
    let promotions_id = promotions.id();
    let default_id = default.id();
    dbg!(&view, &social_id, &promotions_id, &default_id);
    assert!(
        view.available.contains(&social_id),
        "display=true category should be available"
    );
    assert!(
        view.available.contains(&default_id),
        "CategoryDefault should always be available"
    );
    assert!(
        !view.available.contains(&promotions_id),
        "display=false category should not be available"
    );

    let labels = view.into_labels(&tether).await.unwrap();
    let default_label = labels
        .iter()
        .find(|l| l.system_label == SystemLabel::CategoryDefault)
        .unwrap();
    assert!(
        default_label.has_unseen_items,
        "should have unseen items with unread message from disabled promotions category"
    );
}

#[tokio::test]
async fn test_expanded_filter_ids_category_default() {
    let mail_stash = new_test_connection().await;
    let mut tether = mail_stash.connection().await.unwrap();
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
        .tx::<_, _, StashError>(async |bond| {
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

    let mut view = CategoryView::load(&tether).await.unwrap();
    let ids = view
        .enable(Some(default_id))
        .unwrap()
        .query_filter_ids(&tether)
        .await
        .unwrap();

    assert!(
        ids.contains(&default_id),
        "CategoryDefault itself must be in expanded filter"
    );
    assert!(
        ids.contains(&social_id),
        "display=false CategorySocial must be in CategoryDefault expansion"
    );
    assert!(
        !ids.contains(&updates_id),
        "CategoryUpdates cannot be in CategoryDefault expansion"
    );

    let err = view.enable(Some(social_id)).unwrap_err();
    assert!(
        matches!(err, MailContextError::CategoryNotSupported),
        "display=false CategorySocial cannot be enabled for a non-default category"
    );

    let ids = view
        .enable(Some(updates_id))
        .unwrap()
        .query_filter_ids(&tether)
        .await
        .unwrap();

    assert!(
        !ids.contains(&default_id),
        "CategoryDefault cannot be in expanded filter for non-default category"
    );
    assert!(
        !ids.contains(&social_id),
        "display=false CategorySocial cannot be in expanded filter for non-default category"
    );
    assert!(
        ids.contains(&updates_id),
        "CategoryUpdates is enabled category and should only be returned"
    );
}

#[tokio::test]
async fn test_load_returns_empty_when_setting_disabled() {
    let mail_stash = new_test_connection().await;
    let tether = mail_stash.connection().await.unwrap();
    // No MailSettings stored → mail_category_view defaults to false.
    let view = CategoryView::load(&tether).await.unwrap();
    assert!(
        view.available.is_empty(),
        "available should be empty when mail_category_view is disabled"
    );
    assert!(
        view.enabled.is_none(),
        "enabled should be None when mail_category_view is disabled"
    );
}
