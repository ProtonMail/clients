use mail_api::services::proton::common::MessageId;
use mail_api::services::proton::response_data::{
    MailSettings as ApiMailSettings, MessageFlags as ApiMessageFlags,
    MessageMetadata as ApiMessageMetadata, ViewMode as ApiViewMode,
};
use mail_common::Mailbox;
use mail_common::datatypes::SystemLabelId;
use mail_common::models::Message;
use mail_common::test_utils::init::Params as TestParams;
use mail_common::test_utils::test_context::MailTestContext;
use mail_core_api::services::proton::Label as ApiLabel;
use mail_core_api::services::proton::{LabelId, LabelType as ApiLabelType};
use mail_stash::orm::Model;

#[tokio::test]
async fn test_new_mailbox_sync_conversations() {
    // Set up a user and initialise the inbox
    let ctx = MailTestContext::new().await;
    let mut params = TestParams::default_basic();
    params
        .labels
        .get_mut(&ApiLabelType::Label)
        .unwrap()
        .push(ApiLabel {
            id: LabelId::from("testlabel"),
            parent_id: None,
            name: "testlabel".to_owned(),
            path: None,
            color: String::new(),
            label_type: ApiLabelType::Label,
            notify: false,
            display: false,
            sticky: false,
            expanded: false,
            order: 0,
        });
    let conversations = params.conversations.clone();
    ctx.setup_user(params.clone()).await;
    ctx.mock_get_conversations(conversations, 2_u64).await;

    let user_ctx = ctx.mail_user_context().await;

    // Create a mailbox
    let mailbox1 = Mailbox::with_remote_id(
        &user_ctx.user_stash().connection().await.unwrap(),
        params.labels.get(&ApiLabelType::Label).unwrap()[0]
            .id
            .clone(),
    )
    .await
    .unwrap();

    // Create another mailbox
    let mailbox2 = Mailbox::with_remote_id(
        &user_ctx.user_stash().connection().await.unwrap(),
        params.labels.get(&ApiLabelType::Label).unwrap()[1]
            .id
            .clone(),
    )
    .await
    .unwrap();

    // Sync mailbox 1 - this should fire a network request
    mailbox1
        .sync(
            &mut user_ctx.user_stash().connection().await.unwrap(),
            user_ctx.session(),
            10,
        )
        .await
        .unwrap();

    // Sync mailbox 2 - this should also fire a network request
    mailbox2
        .sync(
            &mut user_ctx.user_stash().connection().await.unwrap(),
            user_ctx.session(),
            10,
        )
        .await
        .unwrap();

    // Try syncing mailbox1 again - this should not fire any network requests
    mailbox1
        .sync(
            &mut user_ctx.user_stash().connection().await.unwrap(),
            user_ctx.session(),
            10,
        )
        .await
        .unwrap();

    // Try syncing mailbox2 again - this should not fire any network requests
    mailbox2
        .sync(
            &mut user_ctx.user_stash().connection().await.unwrap(),
            user_ctx.session(),
            10,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn test_new_mailbox_sync_messages() {
    // Set up a user and initialise the inbox
    let ctx = MailTestContext::new().await;
    let mut params = TestParams::default_basic();
    params.mail_settings = Some(ApiMailSettings {
        view_mode: ApiViewMode::Messages,
        ..Default::default()
    });

    let messages = vec![ApiMessageMetadata {
        id: MessageId::from("MyRemoteId"),
        conversation_id: params.conversations[0].id.clone(),
        order: 0,
        address_id: params.addresses[0].id.clone(),
        label_ids: vec![LabelId::inbox()],
        external_id: None,
        subject: "foo".to_owned(),
        sender: Default::default(),
        to_list: vec![],
        cc_list: vec![],
        bcc_list: vec![],
        flags: ApiMessageFlags::empty(),
        time: 0,
        size: 0,
        unread: false,
        is_replied: false,
        is_replied_all: false,
        is_forwarded: false,
        expiration_time: 0,
        snooze_time: 0,
        num_attachments: 0,
        attachments_metadata: vec![],
    }];

    params
        .labels
        .get_mut(&ApiLabelType::Label)
        .unwrap()
        .push(ApiLabel {
            id: LabelId::from("testlabel"),
            parent_id: None,
            name: "testlabel".to_owned(),
            path: None,
            color: String::new(),
            label_type: ApiLabelType::Label,
            notify: false,
            display: false,
            sticky: false,
            expanded: false,
            order: 0,
        });
    ctx.setup_user(params.clone()).await;
    ctx.mock_get_message_metadata(messages, 2_u64).await;

    let user_ctx = ctx.mail_user_context().await;

    // Create a mailbox
    let mailbox1 = Mailbox::with_remote_id(
        &user_ctx.user_stash().connection().await.unwrap(),
        params.labels.get(&ApiLabelType::Label).unwrap()[0]
            .id
            .clone(),
    )
    .await
    .unwrap();

    // Create another mailbox
    let mailbox2 = Mailbox::with_remote_id(
        &user_ctx.user_stash().connection().await.unwrap(),
        params.labels.get(&ApiLabelType::Label).unwrap()[1]
            .id
            .clone(),
    )
    .await
    .unwrap();

    // Sync mailbox 1 - this should fire a network request
    mailbox1
        .sync(
            &mut user_ctx.user_stash().connection().await.unwrap(),
            user_ctx.session(),
            10,
        )
        .await
        .unwrap();

    // Sync mailbox 2 - this should also fire a network request
    mailbox2
        .sync(
            &mut user_ctx.user_stash().connection().await.unwrap(),
            user_ctx.session(),
            10,
        )
        .await
        .unwrap();

    // Try syncing mailbox1 again - this should not fire any network requests
    mailbox1
        .sync(
            &mut user_ctx.user_stash().connection().await.unwrap(),
            user_ctx.session(),
            10,
        )
        .await
        .unwrap();

    // Try syncing mailbox2 again - this should not fire any network requests
    mailbox2
        .sync(
            &mut user_ctx.user_stash().connection().await.unwrap(),
            user_ctx.session(),
            10,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn test_new_mailbox_always_sync_messages_for_drafts_and_sent() {
    // Set up a user and initialise the inbox
    let ctx = MailTestContext::new().await;
    let mut params = TestParams::default_basic();
    // For view mode to conversations.

    params.mail_settings = Some(ApiMailSettings {
        view_mode: ApiViewMode::Conversations,
        ..Default::default()
    });

    let messages = vec![ApiMessageMetadata {
        id: MessageId::from("MyRemoteId"),
        conversation_id: params.conversations[0].id.clone(),
        order: 0,
        address_id: params.addresses[0].id.clone(),
        label_ids: vec![LabelId::drafts(), LabelId::sent()],
        external_id: None,
        subject: "foo".to_owned(),
        sender: Default::default(),
        to_list: vec![],
        cc_list: vec![],
        bcc_list: vec![],
        flags: ApiMessageFlags::empty(),
        time: 0,
        size: 0,
        unread: false,
        is_replied: false,
        is_replied_all: false,
        is_forwarded: false,
        expiration_time: 0,
        snooze_time: 0,
        num_attachments: 0,
        attachments_metadata: vec![],
    }];

    params
        .labels
        .get_mut(&ApiLabelType::Label)
        .unwrap()
        .push(ApiLabel {
            id: LabelId::from("testlabel"),
            parent_id: None,
            name: "testlabel".to_owned(),
            path: None,
            color: String::new(),
            label_type: ApiLabelType::Label,
            notify: false,
            display: false,
            sticky: false,
            expanded: false,
            order: 0,
        });
    ctx.setup_user(params.clone()).await;
    ctx.mock_get_message_metadata(messages, 2_u64).await;

    let user_ctx = ctx.mail_user_context().await;

    // Create a drafts mailbox
    let mailbox_drafts = Mailbox::with_remote_id(
        &user_ctx.user_stash().connection().await.unwrap(),
        LabelId::drafts(),
    )
    .await
    .unwrap();

    // Create sent mailbox
    let mailbox_sent = Mailbox::with_remote_id(
        &user_ctx.user_stash().connection().await.unwrap(),
        LabelId::sent(),
    )
    .await
    .unwrap();

    // Check that mailboxes always sync messages.
    mailbox_drafts
        .sync(
            &mut user_ctx.user_stash().connection().await.unwrap(),
            user_ctx.session(),
            10,
        )
        .await
        .unwrap();

    mailbox_sent
        .sync(
            &mut user_ctx.user_stash().connection().await.unwrap(),
            user_ctx.session(),
            10,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn resolve_message_fetches_missing_dependenceis() {
    // Set up a user and initialise the inbox
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    let new_label_id = LabelId::from("NEW_LABEL");

    let new_label = ApiLabel {
        id: new_label_id.clone(),
        name: "testlabel2".to_owned(),
        label_type: ApiLabelType::Label,
        ..ApiLabel::test_default()
    };

    let message_id = MessageId::from("m1");

    let messages = vec![ApiMessageMetadata {
        id: message_id.clone(),
        conversation_id: params.conversations[0].id.clone(),
        order: 0,
        address_id: params.addresses[0].id.clone(),
        label_ids: vec![LabelId::inbox(), new_label_id.clone()],
        ..ApiMessageMetadata::test_default()
    }];

    ctx.setup_user(params.clone()).await;
    ctx.mock_get_labels_by_ids(vec![new_label]).await;
    ctx.mock_get_message_metadata(messages, 1).await;

    let user_ctx = ctx.mail_user_context().await;

    Message::find_or_fetch_by_remote_id(&user_ctx, message_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn sync_metadata_from_push_notification_always_fetches_from_api() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();

    let message_id = MessageId::from("m1");

    let initial_messages = vec![ApiMessageMetadata {
        id: message_id.clone(),
        conversation_id: params.conversations[0].id.clone(),
        order: 0,
        address_id: params.addresses[0].id.clone(),
        label_ids: vec![LabelId::inbox()],
        ..ApiMessageMetadata::test_default()
    }];

    ctx.setup_user(params.clone()).await;
    // First call: message doesn't exist locally, fetch from API
    ctx.mock_get_message_metadata(initial_messages, 1).await;

    let user_ctx = ctx.mail_user_context().await;

    let msg = Message::sync_metadata_from_push_notification(&user_ctx, message_id.clone())
        .await
        .unwrap();

    assert!(msg.label_ids.contains(&LabelId::inbox()));

    // Second call: message already exists locally.
    // Unlike find_or_fetch_by_remote_id, this should still hit the API.
    ctx.mock_server().reset().await;
    ctx.mock_get_message_metadata(
        vec![ApiMessageMetadata {
            id: message_id.clone(),
            conversation_id: params.conversations[0].id.clone(),
            order: 0,
            address_id: params.addresses[0].id.clone(),
            label_ids: vec![LabelId::inbox()],
            ..ApiMessageMetadata::test_default()
        }],
        1,
    )
    .await;

    let msg2 = Message::sync_metadata_from_push_notification(&user_ctx, message_id)
        .await
        .unwrap();

    assert_eq!(msg.id(), msg2.id());
}
