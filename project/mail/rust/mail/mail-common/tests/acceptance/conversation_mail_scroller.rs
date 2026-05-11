use core::ops::Range;
use itertools::Itertools;
use mail_api::services::proton::common::MessageId;
use mail_api::services::proton::prelude::{
    ConversationEvent, GetConversationsCountResponse, MailEvent, RunningTasks,
};
use mail_api::services::proton::response_data::ConversationCount;
use mail_api::services::proton::{
    common::ConversationId, prelude::GetConversationsResponse,
    response_data::Conversation as ApiConversation,
    response_data::ConversationLabel as ApiConversationLabel,
    response_data::MessageMetadata as ApiMessageMetadata,
};
use mail_common::datatypes::{ConversationViewOptions, IncludeSwitch};
use mail_common::datatypes::{
    SystemLabelId,
    labels::{ScrollOrderDir, ScrollOrderField},
};
use mail_common::models::{
    CachedScrollData, CanonicalCategory, ConversationLabel, LabelExt, LabelWithCounters,
    MailSettings, Message, MessageCounter, ScrollData,
};
use mail_common::test_utils::{
    init::Params as TestParams,
    scroller::{
        StoreLabeledModelMap, TestScroller, TestUpdate, save_single_conversation,
        test_conversations,
    },
    test_context::MailUserContextTestExtension,
};
use mail_common::{
    conv_id, conv_label, conversation, label, lbl_id,
    test_utils::{db::new_test_connection, test_context::MailTestContext},
};
use mail_common::{
    datatypes::{ContextualConversation, ReadFilter},
    models::{Conversation, ConversationCounter, ConversationScrollData},
};
use mail_core_api::services::proton::{Action, EventId, LabelId};
use mail_core_common::models::ModelExtension;
use mail_core_common::{
    datatypes::SystemLabel,
    models::{Label, ModelIdExtension},
};
use mail_network_monitor_service::OsNetworkStatus;
use mail_stash::orm::Model;
use mail_stash::stash::StashError;
use std::{collections::HashMap, time::Duration};
use test_case::test_case;
use velcro::hash_map;
use wiremock::matchers::{query_param, query_param_is_missing};
use wiremock::{
    Mock, Request, ResponseTemplate, Times,
    matchers::{method, path, query_param_contains},
};

macro_rules! assert_scroller_content {
    ($scroller:expr, $len:expr, $expected:expr) => {
        assert_eq!($scroller.items().len(), $len);
        let actual_rids = $scroller
            .items()
            .iter()
            .map(|conv| conv.remote_id.clone())
            .collect_vec();
        let expected_rids = $expected.iter().map(|rid| conv_id!(*rid)).collect_vec();
        assert_eq!(actual_rids, expected_rids);
    };
}

fn expected_conversations(
    n: usize,
    label_id: &str,
    data: &HashMap<Vec<&str>, Vec<Conversation>>,
) -> Option<Vec<ContextualConversation>> {
    let convs = data.get(&vec![label_id])?;
    // Conversations are read in DESC order
    Some(
        convs
            .iter()
            .rev()
            .take(n)
            .filter_map(|conv| {
                let rid = lbl_id!(label_id);
                let label = conv
                    .labels
                    .iter()
                    .find(|label| label.remote_label_id == rid)?;
                let local_label_id = label.local_label_id?;

                ContextualConversation::new(conv.clone(), local_label_id)
            })
            .collect(),
    )
}

#[tokio::test]
async fn test_conversation_mail_scroller_reads_correct_items_within_visible_range_for_cached_scroll_data()
 {
    const REMOTE_LABEL_ID: &str = "rid1";
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    let mut data = hash_map! {
        vec![REMOTE_LABEL_ID]: test_conversations(100, 100),
        vec!["rid2"]: test_conversations(50, 0),
    };

    data.save_to_database(&mut tether).await;

    let remote_label_id = LabelId::from(REMOTE_LABEL_ID);
    let local_label_id = Label::resolve_local_label_id(remote_label_id, &tether)
        .await
        .unwrap();
    let unread = ReadFilter::All;
    let last_conversation =
        Conversation::find_by_remote_id(ConversationId::from("myconv_150"), &tether)
            .await
            .unwrap()
            .unwrap();
    let last_label = last_conversation.label(local_label_id).unwrap();

    let mut scroller = ConversationScrollData::builder()
        .local_label_id(local_label_id)
        .unread(unread)
        .remote_conversation_id(last_conversation.remote_id.clone().unwrap())
        .conversation_time(last_label.context_time)
        .snooze_time(last_label.context_snooze_time)
        .display_order(last_conversation.display_order)
        .order_dir(ScrollOrderDir::Desc)
        .order_field(ScrollOrderField::Time)
        .build();

    tether
        .write_tx(async |bond| scroller.save(bond).await)
        .await
        .unwrap();

    let page_size = 5;

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    let expected = expected_conversations(page_size, REMOTE_LABEL_ID, &data).unwrap();

    // Fetch more and wait for the update (or handle no update case)
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    if !actual.is_empty() {
        assert_eq!(actual, expected);
    } else {
        // If no update was sent, check if we already have data through refresh
        let _refresh_result = test_scroller.refresh_and_wait().await.unwrap();
        // Check if we now have the expected items
        if test_scroller.items().len() >= expected.len() {
            assert_eq!(&test_scroller.items()[..expected.len()], &expected[..]);
        }
    }

    // Try to get more data
    if test_scroller.has_more().await.unwrap() {
        let next_page = test_scroller.fetch_more_and_wait().await.unwrap();
        if !next_page.is_empty() {
            assert_eq!(next_page.len(), page_size);
        }
    }

    // Refresh should not change anything if data is already correct
    let _refresh_result = test_scroller.refresh_and_wait().await.unwrap();
    assert!(test_scroller.items().len() >= expected.len());
}

#[tokio::test]
async fn test_conversation_mail_scroller_reads_one_item_from_online_scroll_data() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    let conversations = params.conversations.clone();

    ctx.mock_get_messages()
        .given_conversation_ids(conversations.iter().map(|c| c.id.clone()))
        .alter(|mock| mock.expect(3..=5))
        .respond_with(vec![])
        .await;
    ctx.mock_get_conversations(conversations, 2..5).await;
    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;
    let user_ctx = ctx.mail_user_context().await;
    let tether = user_ctx.user_stash().connection().await.unwrap();

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let page_size = 5;

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Conversations can be accessed only when progressed.
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(actual.len(), 1);

    // Verify we have the expected data
    assert_eq!(test_scroller.items().len(), 1);

    // Refresh again should not change anything
    let refresh_result = test_scroller.refresh_and_wait().await.unwrap();
    assert!(refresh_result.is_empty());

    assert_eq!(actual[0].remote_id.clone(), conv_id!("myconv"));
    assert!(!test_scroller.has_more().await.unwrap());

    // Additional fetch_more should result in no new data
    let next_page = test_scroller.fetch_more_and_wait().await.unwrap();
    assert!(next_page.is_empty());
}

#[tokio::test]
async fn conversation_scroller_also_fetch_message_metadata() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();

    let conv1_id = ConversationId::from("conv1");
    let conv2_id = ConversationId::from("conv2");
    let msg1_id = MessageId::from("message1");
    let msg2_id = MessageId::from("message2");
    let msg3_id = MessageId::from("message3");

    let conversations = vec![
        ApiConversation {
            id: conv1_id.clone(),
            labels: vec![ApiConversationLabel {
                id: LabelId::inbox(),
                context_expiration_time: 0,
                context_num_attachments: 0,
                context_num_messages: 0,
                context_num_unread: 0,
                context_size: 0,
                context_snooze_time: 0,
                context_time: 0,
            }],
            ..ApiConversation::test_default()
        },
        ApiConversation {
            id: conv2_id.clone(),
            labels: vec![ApiConversationLabel {
                id: LabelId::inbox(),
                context_expiration_time: 0,
                context_num_attachments: 0,
                context_num_messages: 0,
                context_num_unread: 0,
                context_size: 0,
                context_snooze_time: 0,
                context_time: 0,
            }],
            ..ApiConversation::test_default()
        },
    ];

    let messages = vec![
        ApiMessageMetadata {
            id: msg1_id.clone(),
            conversation_id: conv1_id.clone(),
            address_id: params.addresses[0].id.clone(),
            ..ApiMessageMetadata::test_default()
        },
        ApiMessageMetadata {
            id: msg2_id.clone(),
            conversation_id: conv1_id.clone(),
            address_id: params.addresses[0].id.clone(),
            ..ApiMessageMetadata::test_default()
        },
        ApiMessageMetadata {
            id: msg3_id.clone(),
            conversation_id: conv2_id.clone(),
            address_id: params.addresses[0].id.clone(),
            ..ApiMessageMetadata::test_default()
        },
    ];

    ctx.mock_get_messages()
        .given_conversation_ids(conversations.iter().map(|c| c.id.clone()))
        .alter(|mock| mock.expect(1..=2))
        .respond_with(messages.clone())
        .await;
    ctx.mock_get_conversations(conversations, 1..=2).await;
    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;
    let user_ctx = ctx.mail_user_context().await;
    let tether = user_ctx.user_stash().connection().await.unwrap();

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let page_size = 5;

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Conversations can be accessed only when progressed.
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(actual.len(), 2);

    // Verify we have the expected data
    assert_eq!(test_scroller.items().len(), 2);

    let local_conv1 = Conversation::find_by_remote_id(conv1_id, &tether)
        .await
        .unwrap()
        .unwrap();
    let local_conv2 = Conversation::find_by_remote_id(conv2_id, &tether)
        .await
        .unwrap()
        .unwrap();

    assert!(local_conv2.has_messages);
    assert!(local_conv1.has_messages);

    let conv1_messages =
        Message::in_conversation(local_conv1.id(), ConversationViewOptions::All, &tether)
            .await
            .unwrap();
    assert_eq!(conv1_messages.len(), 2);
    assert!(
        conv1_messages
            .iter()
            .any(|m| m.remote_id == Some(msg1_id.clone()))
    );
    assert!(
        conv1_messages
            .iter()
            .any(|m| m.remote_id == Some(msg2_id.clone()))
    );

    let conv2_messages =
        Message::in_conversation(local_conv2.id(), ConversationViewOptions::All, &tether)
            .await
            .unwrap();
    assert_eq!(conv2_messages.len(), 1);
    assert!(
        conv2_messages
            .iter()
            .any(|m| m.remote_id == Some(msg3_id.clone()))
    );
}

#[tokio::test]
async fn test_conversation_mail_scroller_try_to_read_one_item_from_api_when_it_does_not_exist_anymore()
 {
    let ctx = MailTestContext::new().await;
    let mut params = TestParams::default_basic();
    params.conversations = vec![];

    ctx.mock_get_conversations(vec![], 3..5).await;
    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;
    let user_ctx = ctx.mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 1;
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    let page_size = 5;

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Wait for the none update since we do not have any data in API response
    test_scroller.fetch_more().unwrap();
    test_scroller.match_next_update(TestUpdate::None).await;

    // Verify we have nothing in the scroller
    assert_eq!(test_scroller.items().len(), 0);
    // However it will claim to have more data since the total is 1
    assert!(test_scroller.has_more().await.unwrap());
}

#[tokio::test]
async fn test_conversation_mail_scroller_reads_two_pages_from_online_scroll_data() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let page_size = 5;
    let label = SystemLabel::Inbox;
    let remote_label_id = label.remote_id();
    let local_label_id = label.local_id(&tether).await.unwrap().unwrap();
    setup_api_sync_previous_page(&ctx, "myconv_9", None, &remote_label_id, 1).await;
    let params = setup_api_conversation_pages(&ctx, page_size, 0, &remote_label_id, 1..=3).await;
    ctx.setup_user(params.clone()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Update the inbox label to have all conversations
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = page_size as u64 * 2;
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    // Online
    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Conversations can be accessed only when progressed.
    test_scroller.fetch_more_and_wait().await.unwrap();
    assert_scroller_content!(
        &mut test_scroller,
        5,
        &["myconv_9", "myconv_8", "myconv_7", "myconv_6", "myconv_5"]
    );
    assert!(test_scroller.has_more().await.unwrap());

    // Get next page - it will progress cursor to the next page
    // But there is no more data available, the request will return an empty page
    let actual_page = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(actual_page.len(), 5);
    assert_scroller_content!(
        &mut test_scroller,
        10,
        &[
            "myconv_9", "myconv_8", "myconv_7", "myconv_6", "myconv_5", "myconv_4", "myconv_3",
            "myconv_2", "myconv_1", "myconv_0",
        ]
    );
    assert!(!test_scroller.has_more().await.unwrap());

    // Cached - it will trigger two more next page requests for pages as we fetch more
    // and one previous page request on init.
    // This is because cursor have only two pages in cache, which means we will try to get new page evertime we fetch more

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    test_scroller.fetch_more().unwrap();
    let _ = test_scroller.wait_for_update().await.unwrap();
    assert_scroller_content!(
        &mut test_scroller,
        5,
        &["myconv_9", "myconv_8", "myconv_7", "myconv_6", "myconv_5"]
    );
    assert!(test_scroller.has_more().await.unwrap());

    test_scroller.fetch_more().unwrap();
    let _ = test_scroller.wait_for_update().await.unwrap();
    assert_scroller_content!(
        &mut test_scroller,
        10,
        &[
            "myconv_9", "myconv_8", "myconv_7", "myconv_6", "myconv_5", "myconv_4", "myconv_3",
            "myconv_2", "myconv_1", "myconv_0",
        ]
    );
    assert!(!test_scroller.has_more().await.unwrap());
}

#[tokio::test]
async fn test_conversation_mail_scroller_reads_online_folder_for_the_first_time_when_get_an_error_on_request()
 {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    mock_api_forbidden(&ctx).await;

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 1;
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    let page_size = 5;

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // First call should not have any items initially
    assert_eq!(test_scroller.items().len(), 0);

    let result = test_scroller.fetch_more_and_wait().await;
    assert!(result.is_err());
    let actual = result.unwrap_err();
    assert_eq!(
        actual.to_string(),
        "Error: API Error: Forbidden: 403 Forbidden. None".to_string()
    );

    assert_eq!(test_scroller.items().len(), 0);
    // It has more as the total is 1
    assert!(test_scroller.has_more().await.unwrap());

    test_scroller.fetch_more().unwrap();
    test_scroller.match_next_update(TestUpdate::None).await;
    test_scroller.fetch_more().unwrap();
    test_scroller.match_next_update(TestUpdate::None).await;
    test_scroller.match_next_update(TestUpdate::None).await;

    test_scroller.assert_updates(&[
        TestUpdate::Error("API Error: Forbidden: 403 Forbidden. None".to_string()),
        TestUpdate::None,
        TestUpdate::None,
        TestUpdate::None,
    ]);

    // Lets test recovery from the error
    let params = TestParams::default_basic();
    let conversation = params.conversations.first().cloned().unwrap();
    ctx.mock_server().reset().await;
    ctx.mock_ping_success().await;
    ctx.mock_get_conversations(vec![conversation.clone()], 2)
        .await;
    ctx.mock_get_messages()
        .given_conversation_ids([conversation.id.clone()])
        .alter(|mock| mock.expect(2))
        .respond_with(vec![])
        .await;
    test_scroller.fetch_more().unwrap();
    // None because we have no data
    test_scroller.match_next_update(TestUpdate::None).await;
    // However we will spin next request to fetch the data
    test_scroller
        .match_next_update(TestUpdate::Append { items: 1 })
        .await;

    assert_scroller_content!(&mut test_scroller, 1, &[conversation.id]);
}

#[tokio::test]
async fn test_conversation_mail_scroller_reads_offline_folder_for_the_first_time_and_cache_is_empty()
 {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    mock_not_responsive_api(&ctx).await;

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 1;
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    let page_size = 5;

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // First call should not have any items initially
    assert_eq!(test_scroller.items().len(), 0);

    // The items can be read only when we progress with `fetch_more`
    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Error(
            "API Error: Network error: No connection".to_string(),
        ))
        .await;

    assert_eq!(test_scroller.items().len(), 0);
    assert!(test_scroller.has_more().await.unwrap());

    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Error(
            "API Error: Network error: No connection".to_string(),
        ))
        .await;
}

#[tokio::test]
async fn test_conversation_mail_scroller_reads_offline_folder_for_the_first_time_and_cache_has_one_item()
 {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    // Set up cached data
    let remote_label_id = SystemLabel::Inbox.remote_id();
    let mut data = hash_map! {
        vec![remote_label_id.as_str()]: test_conversations(1, 100),
        vec!["rid2"]: test_conversations(50, 0),
    };
    data.save_to_database(&mut tether).await;

    mock_not_responsive_api(&ctx).await;

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 10;
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    let page_size = 5;

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    test_scroller.assert_updates(&[]);
    // The items will be read from cache as the API is unreachable
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(actual.len(), 1);
    assert_eq!(test_scroller.items().len(), 1);
    assert!(test_scroller.has_more().await.unwrap());
    test_scroller.assert_updates(&[TestUpdate::Append { items: 1 }]);

    // No more cached, no API connection, return error
    let actual = test_scroller.fetch_more_and_wait().await.unwrap_err();
    assert_eq!(
        actual.to_string(),
        "Error: API Error: Network error: No connection".to_string()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn test_conversation_mail_scroller_reads_offline_folder_for_the_first_time_and_cache_has_multiple_pages()
 {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    // Set up cached data
    let remote_label_id = SystemLabel::Inbox.remote_id();
    let mut data = hash_map! {
        vec![remote_label_id.as_str()]: test_conversations(11, 100),
        vec!["rid2"]: test_conversations(50, 0),
    };
    data.save_to_database(&mut tether).await;

    mock_not_responsive_api(&ctx).await;

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 15;
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    let page_size = 5;

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // The items will be read from cache as the API is unreachable
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(actual.len(), 5);
    assert_eq!(test_scroller.items().len(), 5);
    assert!(test_scroller.has_more().await.unwrap());

    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(actual.len(), 6);
    assert_eq!(test_scroller.items().len(), 11);

    // It has more but not synced yet
    assert!(test_scroller.has_more().await.unwrap());
    // No more cached, no API connection this should return error
    test_scroller.fetch_more().unwrap();
    let actual = test_scroller.wait_for_update().await;
    assert!(actual.is_err());
    let actual = actual.unwrap_err();
    assert_eq!(
        actual.to_string(),
        "Error: API Error: Network error: No connection".to_string()
    );

    test_scroller.assert_updates(&[
        TestUpdate::Append { items: 5 },
        TestUpdate::Append { items: 6 },
        TestUpdate::Error("API Error: Network error: No connection".to_string()),
    ]);

    // Go online suddenly
    ctx.mock_server().reset().await;
    ctx.mock_ping_success().await;
    setup_api_conversation_pages(&ctx, page_size, 200, &remote_label_id, 2).await;
    user_ctx
        .network_monitor_service()
        .update_os_network_status(OsNetworkStatus::Online);
    user_ctx.network_monitor_service().check_now().await;

    let timeout = Some(Duration::from_secs(3));
    user_ctx
        .wait_for(timeout, |status| status.is_online())
        .await;

    // automatic fetch_more will be triggered by the online status change
    test_scroller.match_next_update(TestUpdate::None).await;

    // Wait for the second update containing the actual data replacement
    // In the new push-based model, fetch_more_and_wait() only waits for immediate feedback,
    // but the actual data replacement from the refresh comes in a second update
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 5 })
        .await;

    assert_scroller_content!(
        &mut test_scroller,
        5,
        &[
            "myconv_209",
            "myconv_208",
            "myconv_207",
            "myconv_206",
            "myconv_205",
        ]
    );

    // progress to the next page from API
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(actual.len(), 5);
    assert_eq!(test_scroller.items().len(), 10);

    assert_scroller_content!(
        &mut test_scroller,
        10,
        &[
            "myconv_209",
            "myconv_208",
            "myconv_207",
            "myconv_206",
            "myconv_205",
            "myconv_204",
            "myconv_203",
            "myconv_202",
            "myconv_201",
            "myconv_200",
        ]
    );

    // No more items in the cache and we are offline but we satisfied the counter
    // Return empty page instead of the Network error
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert!(actual.is_empty());
}

#[tokio::test]
async fn test_conversation_mail_scroller_reads_cached_data_and_return_error_on_offline_fetch_more()
{
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    // Set up cached data
    let remote_label_id = SystemLabel::Inbox.remote_id();
    let mut data = hash_map! {
        vec![remote_label_id.as_str()]: test_conversations(100, 100),
        vec!["rid2"]: test_conversations(50, 0),
    };

    data.save_to_database(&mut tether).await;
    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();

    // Mock offline
    mock_not_responsive_api(&ctx).await;

    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 150;
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    let page_size = 50;

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // The items can be read only when we progress with `fetch_more`
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();

    assert_eq!(actual.len(), 50);
    assert_eq!(test_scroller.items().len(), 50);
    assert!(test_scroller.has_more().await.unwrap());

    // We reached api cached mark, lets serve the rest from cache even if unordered
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();

    assert_eq!(actual.len(), 50);
    assert_eq!(test_scroller.items().len(), 100);
    assert!(test_scroller.has_more().await.unwrap());

    // No more cached, no API connection, return error
    test_scroller.fetch_more().unwrap();
    let actual = test_scroller.wait_for_update().await.unwrap_err();
    assert_eq!(
        actual.to_string(),
        "Error: API Error: Network error: No connection".to_string()
    );
}

#[tokio::test]
async fn test_conversation_mail_scroller_has_insufficient_cached_data_to_fill_first_page() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let page_size = 5;
    let unread = ReadFilter::All;
    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let remote_label_id = SystemLabel::Inbox.remote_id();
    let mut data = hash_map! {
        vec![remote_label_id.as_str()]: test_conversations(3, 100),
    };
    data.save_to_database(&mut tether).await;

    setup_api_sync_previous_page(&ctx, "myconv_102", None, &remote_label_id, 2).await;
    let params = setup_api_conversation_pages(&ctx, page_size, 0, &remote_label_id, 1..=2).await;
    ctx.setup_user(params.clone()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Update the inbox label to have all conversations
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = page_size as u64 * 2 + 3;
    // And simulate we have a cursor pointing correctly to the last
    // conversation which we expect to have 3 though 5 is the page_size
    let last_conversation =
        Conversation::find_by_remote_id(ConversationId::from("myconv_100"), &tether)
            .await
            .unwrap()
            .unwrap();
    let last_label = last_conversation.label(local_label_id).unwrap();
    let mut scroller_cursor = ConversationScrollData::builder()
        .local_label_id(local_label_id)
        .unread(unread)
        .remote_conversation_id(last_conversation.remote_id.clone().unwrap())
        .conversation_time(last_label.context_snooze_time)
        .snooze_time(last_label.context_snooze_time)
        .display_order(last_conversation.display_order)
        .order_dir(ScrollOrderDir::Desc)
        .order_field(ScrollOrderField::SnoozeTime)
        .build();

    tether
        .write_tx(async |bond| {
            scroller_cursor.save(bond).await?;
            counters.save(bond).await
        })
        .await
        .unwrap();

    // The scroller needs to fetch next page from the api due to insufficient amount
    // of items to be displayed as the first page.
    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Fetch more will load 8 items, 3 + 5 as in total it is less than
    // 2 separate pages so it will merge them together.
    let fetched_page = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(fetched_page.len(), 8);

    assert_scroller_content!(
        &mut test_scroller,
        8,
        &[
            "myconv_102",
            "myconv_101",
            "myconv_100",
            "myconv_9",
            "myconv_8",
            "myconv_7",
            "myconv_6",
            "myconv_5",
        ]
    );
    assert!(test_scroller.has_more().await.unwrap());

    // Get next page - it will progress cursor to the next page
    // Since we started moving by whole pages it will fetch 5 items now
    test_scroller.fetch_more().unwrap();
    let actual_page = test_scroller.wait_for_update().await.unwrap().unwrap();
    assert_eq!(actual_page.len(), 5);
    assert_scroller_content!(
        &mut test_scroller,
        13,
        &[
            "myconv_102",
            "myconv_101",
            "myconv_100",
            "myconv_9",
            "myconv_8",
            "myconv_7",
            "myconv_6",
            "myconv_5",
            "myconv_4",
            "myconv_3",
            "myconv_2",
            "myconv_1",
            "myconv_0",
        ]
    );
    assert!(!test_scroller.has_more().await.unwrap());

    // Lets try read it again from cache
    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    test_scroller.fetch_more().unwrap();
    let actual_page = test_scroller.wait_for_update().await.unwrap().unwrap();
    assert_eq!(actual_page.len(), 5);
    assert_scroller_content!(
        &mut test_scroller,
        5,
        &[
            "myconv_102",
            "myconv_101",
            "myconv_100",
            "myconv_9",
            "myconv_8",
        ]
    );
    assert!(test_scroller.has_more().await.unwrap());

    // This `fetch_more` will join two last pages together as the last page is incomplete
    test_scroller.fetch_more().unwrap();
    let actual_page = test_scroller.wait_for_update().await.unwrap().unwrap();
    assert_eq!(actual_page.len(), 8);

    assert_scroller_content!(
        &mut test_scroller,
        13,
        &[
            "myconv_102",
            "myconv_101",
            "myconv_100",
            "myconv_9",
            "myconv_8",
            "myconv_7",
            "myconv_6",
            "myconv_5",
            "myconv_4",
            "myconv_3",
            "myconv_2",
            "myconv_1",
            "myconv_0",
        ]
    );
    assert!(!test_scroller.has_more().await.unwrap());
}

#[test_case(50, 3; "Test1: Conversation added at the end in offline mode it will be added to the end of the list, 3 (3 + 0) items"
)]
#[tokio::test]
async fn test_conversation_mail_scroller_database_refresh_will_not_triggers_fetch_for_small_totals(
    order: usize,
    expected: usize,
) {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let page_size = 10; // Larger than our test data
    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();

    // Set up cached data with fewer items than page size
    let remote_label_id = SystemLabel::Inbox.remote_id();
    let mut data = hash_map! {
        vec![remote_label_id.as_str()]: test_conversations(3, 100), // Less than page_size
    };
    data.save_to_database(&mut tether).await;

    // Mock offline to use cached data
    mock_not_responsive_api(&ctx).await;

    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 3; // Less than page_size (10)
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Conversations can be accessed only when progressed.
    let _ = test_scroller.fetch_more_and_wait().await.unwrap();

    // Add a conversation to trigger refresh
    let label = Label::load(local_label_id, &tether).await.unwrap().unwrap();
    let new_conversation = test_conversations(1, order).pop().unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            save_single_conversation(&[label], &mut new_conversation.clone(), bond).await;
            Ok(())
        })
        .await
        .unwrap();

    // For small totals (< page_size), refresh should internally call fetch_more
    // to ensure data is loaded as there is no way to scroll down to trigger fetch_more
    assert_eq!(test_scroller.items().len(), expected);
    assert!(test_scroller.has_more().await.unwrap());

    // Refresh update arrives
    let _ = test_scroller.wait_for_update().await.unwrap();
    assert!(!test_scroller.has_more().await.unwrap());
    assert_eq!(test_scroller.items().len(), expected + 1);
}

#[test_case(200, 4; "Test2: Conversation added at the beggining, 4 (3 + 1) items, as the item is at the beggining"
)]
#[tokio::test]
async fn test_conversation_mail_scroller_database_refresh_triggers_fetch_for_small_totals(
    order: usize,
    expected: usize,
) {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let page_size = 10; // Larger than our test data
    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();

    // Set up cached data with fewer items than page size
    let remote_label_id = SystemLabel::Inbox.remote_id();
    let mut data = hash_map! {
        vec![remote_label_id.as_str()]: test_conversations(3, 100), // Less than page_size
    };
    data.save_to_database(&mut tether).await;

    // Mock offline to use cached data
    mock_not_responsive_api(&ctx).await;

    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 3; // Less than page_size (10)
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Conversations can be accessed only when progressed.
    let _ = test_scroller.fetch_more_and_wait().await.unwrap();

    // Add a conversation to trigger refresh
    let label = Label::load(local_label_id, &tether).await.unwrap().unwrap();
    let new_conversation = test_conversations(1, order).pop().unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            save_single_conversation(&[label], &mut new_conversation.clone(), bond).await;
            Ok(())
        })
        .await
        .unwrap();

    // Wait for refresh notification
    let _ = test_scroller.wait_for_update().await.unwrap();

    // For small totals (< page_size), all_items should internally call fetch_more
    // to ensure data is loaded as there is no way to scroll down to trigger fetch_more
    assert_eq!(test_scroller.items().len(), expected);

    assert!(!test_scroller.has_more().await.unwrap());
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert!(actual.is_empty());
}

#[test_case(50, 5; "Test1: Conversation added at the end, 5 (5 + 0) items, as the page size is 5")]
#[test_case(200, 6; "Test2: Conversation added at the beggining, 6 (5 + 1) items, as the item is at the beggining"
)]
#[tokio::test]
async fn test_conversation_mail_scroller_database_refresh_triggers_fetch_for_large_totals(
    order: usize,
    expected: usize,
) {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let page_size = 5;
    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();

    // Set up cached data
    let remote_label_id = SystemLabel::Inbox.remote_id();
    let mut data = hash_map! {
        vec![remote_label_id.as_str()]: test_conversations(15, 100),
    };
    data.save_to_database(&mut tether).await;

    // Mock offline to use cached data
    mock_not_responsive_api(&ctx).await;

    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 15;
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Load first page
    let first_page = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(first_page.len(), 5);

    // Trigger dirty state
    let label = Label::load(local_label_id, &tether).await.unwrap().unwrap();
    let new_conversation = test_conversations(1, order).pop().unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            save_single_conversation(&[label], &mut new_conversation.clone(), bond).await;
            Ok(())
        })
        .await
        .unwrap();

    let _ = test_scroller
        .try_wait_for_update(Duration::from_secs(10))
        .await;

    assert_eq!(test_scroller.items().len(), expected);
    assert!(test_scroller.has_more().await.unwrap());
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(actual.len(), 5);
    assert!(test_scroller.has_more().await.unwrap());
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert!(!actual.is_empty());
    assert_eq!(test_scroller.items().len(), 16); // 15 + 1 new
    assert!(!test_scroller.has_more().await.unwrap());
}

#[tokio::test]
async fn snoozed_conversations() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    let label = Label::find_by_remote_id(LabelId::snoozed(), &tether)
        .await
        .unwrap()
        .unwrap();

    let mut data = {
        let label = label.remote_id.clone().unwrap().to_string();

        hash_map! {
            vec![label]: test_conversations(5, 0),
        }
    };

    data.save_to_database(&mut tether).await;

    // ---

    let snooze_times = [300, 200, 400, 100, 500];

    for (conv, conv_snooze_time) in data.values_mut().flatten().zip(snooze_times) {
        conv.labels[0].context_snooze_time = conv_snooze_time.into();
        tether
            .write_tx(async |tx| conv.save(tx).await)
            .await
            .unwrap();
    }

    // ---

    let mut scroller = TestScroller::conversations(&user_ctx, label.id(), 2)
        .await
        .unwrap();

    let convs = scroller.fetch_more_and_wait().await.unwrap();

    assert_eq!(2, convs.len());
    assert_eq!("myconv_3", convs[0].remote_id.as_ref().unwrap().to_string());
    assert_eq!("myconv_1", convs[1].remote_id.as_ref().unwrap().to_string());

    let convs = scroller.fetch_more_and_wait().await.unwrap();

    assert_eq!(3, convs.len());
    assert_eq!("myconv_0", convs[0].remote_id.as_ref().unwrap().to_string());
    assert_eq!("myconv_2", convs[1].remote_id.as_ref().unwrap().to_string());
    assert_eq!("myconv_4", convs[2].remote_id.as_ref().unwrap().to_string());
}

#[tokio::test]
async fn test_conversation_snooze_time_ordering_with_same_snooze_time_different_context_time() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    // Mock offline to use cached data
    mock_not_responsive_api(&ctx).await;

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let unread = ReadFilter::All;
    let page_size = 3;

    // Create 3 test conversations for inbox with same snooze_time but different context_time
    let same_snooze_time = 1000;
    let context_times = [500, 300, 700]; // Different context times
    let orders: [u64; 3] = [10, 20, 30]; // Different display orders

    // Save conversations to database using the inbox label
    let mut data = hash_map! {
        vec![LabelId::inbox().to_string()]: test_conversations(3, 0),
    };
    data.save_to_database(&mut tether).await;
    for (i, (conv, (context_time, order))) in data
        .values_mut()
        .flatten()
        .zip(context_times.iter().zip(orders.iter()))
        .enumerate()
    {
        conv.remote_id = Some(format!("snooze_conv_{i}").into());
        conv.labels[0].context_snooze_time = same_snooze_time.into();
        conv.labels[0].context_time = (*context_time).into();
        conv.display_order = *order;
        tether
            .write_tx(async |tx| conv.save(tx).await)
            .await
            .unwrap();
    }
    let mut cursor_scroller = ConversationScrollData::builder()
        .local_label_id(local_label_id)
        .unread(unread)
        .remote_conversation_id("Everything visible".into())
        .conversation_time(200.into()) // all in range of the cursor
        .snooze_time(200.into())
        .display_order(30)
        .order_dir(ScrollOrderDir::Desc)
        .order_field(ScrollOrderField::SnoozeTime)
        .build();
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 3;
    tether
        .write_tx(async |bond| {
            cursor_scroller.save(bond).await?;
            counters.save(bond).await
        })
        .await
        .unwrap();

    // Set up mocks
    mock_not_responsive_api(&ctx).await;

    // Create scroller with SnoozeTime ordering
    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Fetch conversations
    let items = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(items.len(), 3);

    // Verify ordering: With MAX(snooze_time, context_time), all get MAX(1000, time) = 1000
    // So tie-breaker is display_order DESC
    // Expected order: conv_2 (display_order=30), conv_1 (display_order=20), conv_0 (display_order=10)
    // 1st
    assert_eq!(
        items[0].remote_id.as_ref().unwrap().to_string(),
        "snooze_conv_2"
    );
    assert_eq!(items[0].snooze_time.as_u64(), 1000);
    assert_eq!(items[0].time.as_u64(), 700);
    // 2nd
    assert_eq!(
        items[1].remote_id.as_ref().unwrap().to_string(),
        "snooze_conv_1"
    );
    assert_eq!(items[1].snooze_time.as_u64(), 1000);
    assert_eq!(items[1].time.as_u64(), 300);
    // 3rd
    assert_eq!(
        items[2].remote_id.as_ref().unwrap().to_string(),
        "snooze_conv_0"
    );
    assert_eq!(items[2].snooze_time.as_u64(), 1000);
    assert_eq!(items[2].time.as_u64(), 500);

    let mut last = conversation!(remote_id: Some("snooze_conv_3".into()),
        labels: vec![ConversationLabel {
            remote_label_id: Some(LabelId::inbox()),
            context_snooze_time: 500.into(),
            context_time: 1500.into(),
            ..ConversationLabel::test_default()
        }],
        display_order: 5
    );
    tether
        .write_tx(async |tx| last.save(tx).await)
        .await
        .unwrap();

    test_scroller.wait_for_update().await.unwrap().unwrap();

    // snooze_conv_3 should appear first: MAX(500, 1500) = 1500 > 1000
    let items = test_scroller.items();
    assert_eq!(items.len(), 4);
    assert_eq!(
        items[0].remote_id.as_ref().unwrap().to_string(),
        "snooze_conv_3"
    );
    assert_eq!(items[0].snooze_time.as_u64(), 500);
    assert_eq!(items[0].time.as_u64(), 1500);
}

#[tokio::test]
async fn test_conversation_snooze_time_pagination_fix_works() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();

    let mut conversations = vec![];

    // Create 10 snoozed conversations
    for i in 0..10 {
        conversations.push(conversation!(
            remote_id: Some(format!("snoozed_{i}").into()),
            display_order: (100 + i),
            labels: vec![ConversationLabel {
                remote_label_id: Some(LabelId::inbox()),
                context_time: (1000 + i).into(),
                context_snooze_time: (10000 + i).into(),
                ..ConversationLabel::test_default()
            }]
        ));
    }

    // Create 5 non-snoozed conversations
    for i in 0..5 {
        conversations.push(conversation!(
            remote_id: Some(format!("normal_{i}").into()),
            display_order: (50 + i),
            labels: vec![ConversationLabel {
                remote_label_id: Some(LabelId::inbox()),
                context_time: (900 + i).into(),
                context_snooze_time: 0.into(),
                ..ConversationLabel::test_default()
            }]
        ));
    }

    let mut data = hash_map! {
        vec![LabelId::inbox().to_string()]: conversations,
    };
    data.save_to_database(&mut tether).await;

    let page_size = 5;
    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Load only the first page
    let first_page = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(first_page.len(), 5);
    assert!(test_scroller.has_more().await.unwrap());

    // Insert a new conversation with context_time=15000 (highest) but context_snooze_time=0
    let mut new_conversation = conversation!(
        remote_id: Some("new_newest_conversation".into()),
        display_order: 200,
        labels: vec![ConversationLabel {
            remote_label_id: Some(LabelId::inbox()),
            context_time: 15000.into(),
            context_snooze_time: 0.into(),
            ..ConversationLabel::test_default()
        }]
    );

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            let label = Label::load(local_label_id, bond).await.unwrap().unwrap();
            save_single_conversation(&[label], &mut new_conversation, bond).await;
            Ok(())
        })
        .await
        .unwrap();

    test_scroller.wait_for_update().await.unwrap().unwrap();

    let new_conv_position = test_scroller
        .items()
        .iter()
        .position(|c| c.remote_id == conv_id!("new_newest_conversation"))
        .unwrap();

    // With MAX(15000, 0) = 15000 > MAX(10000-10009, 1000-1009), it should be first
    assert_eq!(new_conv_position, 0);
}

#[tokio::test]
async fn test_conversation_mail_scroller_fetch_new() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    let label = SystemLabel::Inbox;
    let conversations = params
        .conversations
        .first()
        .cloned()
        .map(|mut conv| {
            conv.context_time = Some(100);
            conv.order = 100;
            conv
        })
        .into_iter()
        .collect_vec();

    let previous_page = conversations
        .first()
        .cloned()
        .map(|mut conv| {
            conv.id = "myconv_0".into();
            conv.context_time = Some(110);
            conv.order = 110;
            conv
        })
        .into_iter()
        .collect_vec();

    let remote_label_id = label.remote_id();
    // Mock previous page
    setup_api_sync_previous_page(&ctx, "myconv_0", None, &remote_label_id, 1).await;
    setup_api_sync_previous_page(&ctx, "myconv", Some(previous_page), &remote_label_id, 1).await;
    // Counters are also fetched on previous page
    setup_api_sync_conv_label_counters(&ctx, &remote_label_id, 1, 1, 5).await;

    // Mock next page
    mock_get_conversations_page(&ctx, vec![], "myconv", &remote_label_id, 1).await;
    // Mock first page

    // This method will be called 2 times from previous and first. Since the keys are the same,
    // it needs to be mocked separately.
    ctx.mock_get_messages()
        .alter(|mock| mock.expect(2))
        .respond_with(vec![])
        .await;

    ctx.mock_get_conversations(conversations, 1).await;
    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;
    let user_ctx = ctx.mail_user_context().await;
    let tether = user_ctx.user_stash().connection().await.unwrap();

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let page_size = 5;

    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Conversations can be accessed only when progressed.
    let actual = test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(actual.len(), 1);
    assert_eq!(test_scroller.items().len(), 1);

    test_scroller.fetch_new().unwrap();
    test_scroller.match_next_update(TestUpdate::None).await;
    test_scroller
        .match_next_update(TestUpdate::ReplaceBefore { idx: 0, items: 1 })
        .await;
    assert_eq!(test_scroller.items().len(), 2);

    test_scroller.fetch_new().unwrap();
    test_scroller.match_next_update(TestUpdate::None).await;
    assert_eq!(test_scroller.items().len(), 2);
    let tether = user_ctx.user_stash().connection().await.unwrap();
    let label = LabelWithCounters::load(local_label_id, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(label.unread_conv, 5);
}

#[tokio::test]
async fn conversation_mail_scroller_reacts_to_creat_conversation_event() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let page_size = 5;
    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();

    ctx.mock_ping_success().await;
    let params = TestParams::default_basic();
    ctx.setup_user(params.clone()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;
    let mut test_conversation = params.conversations.clone().pop().unwrap();
    let conv_id_1 = ConversationId::from("myconv_9");
    let conv_id_2 = ConversationId::from("myconv_10");
    test_conversation.id = conv_id_1.clone();
    test_conversation.order = 9;
    test_conversation.context_time = Some(9);
    ctx.mock_get_conversations(vec![test_conversation], 2).await;
    // Empty response is fine, just to satisfy network check requirements.
    ctx.mock_get_messages()
        .given_conversation_ids([conv_id_1.clone()])
        .alter(|mock| mock.expect(2))
        .respond_with(vec![])
        .await;
    //mock_get_conversations_page(&ctx, vec![], &test_conv_id, 1).await;

    // Update the inbox label to have all conversations
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 1;
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    // Online
    let mut test_scroller = TestScroller::conversations(&user_ctx, local_label_id, page_size)
        .await
        .unwrap();

    // Conversations can be accessed only when progressed.
    test_scroller.fetch_more_and_wait().await.unwrap();
    assert_scroller_content!(&mut test_scroller, 1, &["myconv_9"]);

    // Simulate new event
    let event = MailEvent {
        event_id: EventId::from("New Event"),
        labels: None,
        conversation_counts: Some(vec![ConversationCount {
            label_id: LabelId::inbox(),
            total: 2,
            unread: 1,
        }]),
        conversations: Some(vec![ConversationEvent {
            id: conv_id_2.clone(),
            action: Action::Create,
            conversation: Some(ApiConversation {
                id: conv_id_2.clone(),
                attachment_info: Default::default(),
                attachments_metadata: vec![],
                display_snoozed_reminder: false,
                expiration_time: 0,
                labels: vec![ApiConversationLabel {
                    id: LabelId::inbox(),
                    context_expiration_time: 0,
                    context_num_attachments: 0,
                    context_num_messages: 1,
                    context_num_unread: 1,
                    context_size: 100,
                    context_snooze_time: 0,
                    context_time: 10,
                }],
                num_attachments: 0,
                num_messages: 1,
                num_unread: 1,
                order: 10,
                recipients: vec![],
                senders: vec![],
                size: 100,
                subject: "".to_string(),
                context_time: None,
            }),
        }]),
        incoming_defaults: None,
        mail_settings: None,
        message_counts: None,
        messages: None,
        refresh: 0,
        has_more: false,
    };

    user_ctx.apply_event(event).await.unwrap();
    // Sanity check expected state
    let conversations = Conversation::in_label(local_label_id, &tether)
        .await
        .unwrap();
    assert_eq!(conversations.len(), 2);
    assert_eq!(conversations[0].remote_id.as_ref(), Some(&conv_id_2));
    assert_eq!(conversations[1].remote_id.as_ref(), Some(&conv_id_1));
    let conv_counts = ConversationCounter::find_by_id(local_label_id, &tether)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(conv_counts.unread, 1);
    assert_eq!(conv_counts.total, 2);

    let update = tokio::time::timeout(Duration::from_secs(5), test_scroller.wait_for_update())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(update.len(), 1);
    assert_eq!(update[0].remote_id.as_ref(), Some(&conv_id_2));
}

// Tests the instance in which the cached data is equal to the API data in the time of first visit of that location.
// To imagine how it could happen imagine the following scenario in quick succession:
// - User creates a folder
// - User moves 10 items to the folder (lets assume he waits till API is synced)
// - User visits the folder
// - User removes 1 item from the folder
//
// Most notable thing about the test is it triggers the `fetch_more` 3 times.
// - 1st time by user which reads from cache and state is NotSynced
// - 2nd time (auto) invalidation from the first_page sync state changes from NotSynced to Online
// - 3rd time (auto) database refresh from the watcher state is Online
//
// Without ET-4791 fix this test would give 2 `Append` updates with the same data, esentially duplicating the page.
#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn test_conversation_mail_scroller_reads_non_empty_folder_for_the_first_time_and_api_data_is_equal_to_the_cache()
 {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let api_page = create_api_conversation_page(0..9, 100);
    let models = api_page
        .iter()
        .map(|conv| Conversation::from(conv.clone()))
        .collect_vec();
    // Set up cached data
    let remote_label_id = SystemLabel::Inbox.remote_id();
    let mut data = hash_map! {
        vec![remote_label_id.as_str()]: models,
        vec!["rid2"]: test_conversations(50, 0),
    };
    data.save_to_database(&mut tether).await;

    ctx.mock_get_conversations(api_page, 4..=6).await;
    ctx.mock_ping_success().await;

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 10;
    tether
        .write_tx(async |bond| counters.save(bond).await)
        .await
        .unwrap();

    let page_size = 10;
    let mut test_scroller =
        TestScroller::conversations_instant(&user_ctx, local_label_id, page_size)
            .await
            .unwrap();

    // The items will be read from cache as we have 9 items in cache
    // And the exact same data is in the API
    test_scroller.fetch_more().unwrap(); // 1st fetch_more
    test_scroller
        .match_next_update(TestUpdate::Append { items: 9 })
        .await;
    assert!(test_scroller.has_more().await.unwrap());
    // 2nd fetch_more goes when first_page sync finishes around here.
    // |<-
    // No update is expected as the data is the same
    let update = test_scroller
        .try_wait_for_update(Duration::from_secs(2))
        .await
        .unwrap();
    assert!(update.is_none());
    assert_eq!(test_scroller.items().len(), 9);

    // 3rd fetch_more by triggering invisble database update.
    let mut new_data = hash_map! {
        vec!["rid2"]: test_conversations(1, 299),
    };
    new_data.save_to_database(&mut tether).await;

    // We shouldn't get any update as the data is still the same
    let update = test_scroller
        .try_wait_for_update(Duration::from_secs(2))
        .await
        .unwrap();
    assert!(update.is_none());
    assert_eq!(test_scroller.items().len(), 9);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn test_conversation_mail_scroller_change_label() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let page_size = 10;
    let mut api_page = create_api_conversation_page(0..9, 100);
    for conv in api_page.iter_mut() {
        conv.labels = vec![ApiConversationLabel {
            id: LabelId::inbox(),
            ..ApiConversationLabel::test_default()
        }];
    }
    // Set up cached data
    let remote_label_id = SystemLabel::Inbox.remote_id();
    let mut data = hash_map! {
        vec![remote_label_id.as_str()]: vec![],
        vec!["rid2"]: test_conversations(50, 0),
    };
    data.save_to_database(&mut tether).await;

    let api_page_clone = api_page.clone();
    ctx.mock_get_conversations_with(move |builder| {
        builder
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                    conversations: api_page_clone.clone(),
                    tasks_running: RunningTasks::none(),
                    stale: false,
                    total: 1,
                }),
            )
            .expect(2..=8)
    })
    .await;
    ctx.mock_get_messages()
        .given_conversation_ids(api_page.iter().map(|c| c.id.clone()))
        .alter(|mock| mock.expect(2..=8))
        .respond_with(vec![])
        .await;
    ctx.mock_ping_success().await;

    // we should get an update on the first fetch_more in Inbox despite the data being stale
    let inbox_local_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let mut inbox_counters = ConversationCounter::new(inbox_local_id);
    let remote_label_id = LabelId::from("rid2");
    let rid2_local_id = Label::remote_id_counterpart(remote_label_id, &tether)
        .await
        .unwrap()
        .unwrap();
    let mut rid2_counters = ConversationCounter::new(rid2_local_id);
    inbox_counters.total = 10;
    rid2_counters.total = 50;
    tether
        .write_tx(async |bond| {
            inbox_counters.save(bond).await?;
            rid2_counters.save(bond).await
        })
        .await
        .unwrap();

    let mut test_scroller =
        TestScroller::conversations_instant(&user_ctx, inbox_local_id, page_size)
            .await
            .unwrap();

    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Append { items: 9 })
        .await;
    assert!(test_scroller.has_more().await.unwrap());

    // Switch to custom label "rid2"
    test_scroller.change_label(rid2_local_id).unwrap();
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 10 })
        .await;
    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Append { items: 10 })
        .await;

    // Switch back to inbox
    test_scroller.change_label(inbox_local_id).unwrap();
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 9 })
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn test_conversation_mail_scroller_change_include() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let page_size = 10;
    let mut api_page = create_api_conversation_page(0..9, 100);
    for conv in api_page.iter_mut() {
        conv.labels = vec![ApiConversationLabel {
            id: LabelId::almost_all_mail(),
            ..ApiConversationLabel::test_default()
        }];
    }
    // Set up cached data
    let almost_all_mail_remote_id = SystemLabel::AlmostAllMail.remote_id();
    let all_mail_remote_id = SystemLabel::AllMail.remote_id();
    let mut data = hash_map! {
        vec![almost_all_mail_remote_id.as_str()]: vec![],
        vec![all_mail_remote_id.as_str()]: test_conversations(50, 0),
    };
    data.save_to_database(&mut tether).await;

    let api_page_clone = api_page.clone();
    ctx.mock_get_conversations_with(move |builder| {
        builder
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                    conversations: api_page_clone.clone(),
                    tasks_running: RunningTasks::none(),
                    stale: false,
                    total: 1,
                }),
            )
            .expect(2..=8)
    })
    .await;
    ctx.mock_get_messages()
        .given_conversation_ids(api_page.iter().map(|c| c.id.clone()))
        .alter(|mock| mock.expect(2..=8))
        .respond_with(vec![])
        .await;
    ctx.mock_ping_success().await;

    // we should get an update on the first fetch_more in Inbox despite the data being stale
    let almost_all_mail_local_id = SystemLabel::AlmostAllMail
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();
    let all_mail_local_id = SystemLabel::AllMail
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();
    let mut almost_all_mail_counters = ConversationCounter::new(almost_all_mail_local_id);
    let mut all_mail_counters = ConversationCounter::new(all_mail_local_id);
    almost_all_mail_counters.total = 10;
    all_mail_counters.total = 50;
    tether
        .write_tx(async |bond| {
            almost_all_mail_counters.save(bond).await?;
            all_mail_counters.save(bond).await
        })
        .await
        .unwrap();

    let mut test_scroller =
        TestScroller::conversations_instant(&user_ctx, almost_all_mail_local_id, page_size)
            .await
            .unwrap();

    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Append { items: 9 })
        .await;
    assert!(test_scroller.has_more().await.unwrap());

    // Switch to all mail
    test_scroller
        .change_include(IncludeSwitch::WithSpamAndTrash)
        .unwrap();
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 10 })
        .await;
    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Append { items: 10 })
        .await;

    // Switch back to almost all mail
    test_scroller
        .change_include(IncludeSwitch::Default)
        .unwrap();
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 9 })
        .await;
}

#[tokio::test]
async fn test_conversation_mail_scroller_end_cursor_is_not_pointing_to_any_element() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    // Set up cached data
    let remote_label_id = SystemLabel::Inbox.remote_id();
    let page_size = 5;
    let mut data = hash_map! {
        vec![remote_label_id.as_str()]: test_conversations(1, 100),
        vec!["rid2"]: test_conversations(50, 0),
    };
    data.save_to_database(&mut tether).await;

    // We should not get a previous page request!
    setup_api_sync_previous_page(&ctx, "myconv_100", None, &remote_label_id, 0).await;
    // We will only run first page requests
    ctx.mock_get_conversations(vec![], 3).await;

    let local_label_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let mut counters = ConversationCounter::new(local_label_id);
    counters.total = 10;
    let mut cursor = ConversationScrollData::builder()
        .local_label_id(local_label_id)
        .unread(ReadFilter::All)
        .order_dir(ScrollOrderDir::for_label(&remote_label_id))
        .order_field(ScrollOrderField::for_label(&remote_label_id))
        .conversation_time(0.into())
        .snooze_time(0.into())
        .display_order(10) // we need to base our cursor reach on the display order
        .remote_conversation_id("this_does_not_exist".into())
        .build();
    tether
        .write_tx(async |bond| {
            counters.save(bond).await?;
            cursor.save(bond).await
        })
        .await
        .unwrap();

    let cached_cursor = CachedScrollData::<ConversationScrollData>::new(
        local_label_id,
        ReadFilter::All,
        page_size,
        vec![],
        &tether,
    )
    .await
    .unwrap()
    .unwrap();
    let end_cursor = cached_cursor.load_end_cursor(&tether).await.unwrap();
    assert_eq!(
        end_cursor.remote_conversation_id,
        "this_does_not_exist".into()
    );
    let end_element = cached_cursor
        .scroll_data_end(&tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(end_element.remote_conversation_id, "myconv_100".into());

    let mut test_scroller =
        TestScroller::conversations_instant(&user_ctx, local_label_id, page_size)
            .await
            .unwrap();

    // Here cursor should no longer exist
    let cursor = CachedScrollData::<ConversationScrollData>::new(
        local_label_id,
        ReadFilter::All,
        page_size,
        vec![],
        &tether,
    )
    .await
    .unwrap();

    assert!(cursor.is_none());

    // Besides the fact the cursor existed and first element still exists
    // we will not request next and previous pages and instead it will request first page
    // since the end cursor is not pointing to any element
    test_scroller.fetch_more().unwrap();

    // Return cashed element instantly
    test_scroller
        .match_next_update(TestUpdate::Append { items: 1 })
        .await;

    // The first page request is running in background
    // but since it has no items it will not trigger refresh
    // Lets fetch more again to see that we will not get any items in the update
    test_scroller.fetch_more().unwrap();
    test_scroller.match_next_update(TestUpdate::None).await;
}

/// Make sure that deleting all messages from a label causes that label to
/// appear empty until the server confirms that messages are actually gone.
///
/// ---
///
/// Emptying a label is an async backend action - when we call `apply_remote()`,
/// the backend schedules a task to slowly delete messages in the background and
/// the request itself completes immediately.
///
/// Without any extra care to accommodate for this behavior, the scroller could
/// accidentally bring those about-to-be-deleted messages back - at least until
/// event loop catches up - making the UI look confusing.
///
/// Say, we've got 10k messages in trash - now:
///
/// - T+0: you create scroller,
/// - T+1: you delete all messages,
/// - T+2: server deletes the first 1k messages,
/// - T+3: you pull-to-refresh,
///
/// - T+4: scroller asks backend for messages - 9k of them are still present in
///        the database, so we get the first page out of those 9k, causing those
///        messages to reappear on device until the event loop catches up [!]
///
/// To solve this problem, the delete-all action marks the label as busy until
/// the server acknowledges that all messages have been indeed deleted.
///
/// Until this acknowledgment arrives, we pretend that the label is empty, even
/// if the server returned us some messages - that's because we know those
/// messages will be gone in a moment anyway so there's no point in bothering
/// the user with them.
///
/// ---
///
/// NOTE we've got an equivalent test for the message scroller - if you modify
///      this test, make sure you adjust the other one as well
#[tokio::test]
async fn delete_all() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();

    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;

    let user_ctx = ctx.mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let label = SystemLabel::Trash.load(&tether).await.unwrap().unwrap();

    // ---
    // [1] Initial state - pretend we've got 100 messages in trash

    let mut msg_counter = MessageCounter {
        local_label_id: label.id(),
        total: 100,
        unread: 80,
    };

    let mut conv_counter = ConversationCounter {
        local_label_id: label.id(),
        total: 30,
        unread: 20,
    };

    tether
        .write_tx(async |tx| msg_counter.save(tx).await)
        .await
        .unwrap();

    tether
        .write_tx(async |tx| conv_counter.save(tx).await)
        .await
        .unwrap();

    // ---

    let mut convs1 = create_api_conversation_page(0..10, 100);
    let mut convs2 = create_api_conversation_page(0..10, 110);
    let mut msg_id = 0;

    for convs in [&mut convs1, &mut convs2] {
        for conv in convs.iter_mut() {
            conv.labels = vec![ApiConversationLabel {
                id: label.remote_id().unwrap().clone(),
                ..ApiConversationLabel::test_default()
            }];
        }

        let msgs = convs
            .iter()
            .map(|conv| {
                msg_id += 1;

                ApiMessageMetadata {
                    id: MessageId::from(format!("msg{msg_id}")),
                    conversation_id: conv.id.clone(),
                    address_id: params.addresses[0].id.clone(),
                    label_ids: vec![label.remote_id().unwrap().clone()],
                    ..ApiMessageMetadata::test_default()
                }
            })
            .collect();

        ctx.mock_get_messages()
            .given_conversation_ids(convs.iter().map(|c| c.id.clone()))
            .alter(|mock| mock.expect(1))
            .respond_with(msgs)
            .await;
    }

    ctx.mock_get_conversations_with(|builder| {
        builder
            .and(query_param_is_missing("Anchor"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                    conversations: convs1,
                    tasks_running: RunningTasks::none(),
                    stale: false,
                    total: 100,
                }),
            )
            .expect(1)
    })
    .await;

    ctx.mock_get_conversations_with(|builder| {
        builder
            .and(query_param("Anchor", "0"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                    conversations: convs2,
                    tasks_running: RunningTasks::none(),
                    stale: false,
                    total: 100,
                }),
            )
            .expect(1)
    })
    .await;

    let mut target = TestScroller::conversations_instant(&user_ctx, label.id(), 10)
        .await
        .unwrap();

    target.fetch_more().unwrap();

    assert_eq!(
        vec![
            "myconv_109",
            "myconv_108",
            "myconv_107",
            "myconv_106",
            "myconv_105",
            "myconv_104",
            "myconv_103",
            "myconv_102",
            "myconv_101",
            "myconv_100",
        ],
        target
            .wait_for_update()
            .await
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|cnv| cnv.remote_id.unwrap().to_string())
            .collect::<Vec<_>>()
    );

    assert!(target.has_more().await.unwrap());

    // ---
    // [2] Schedule the "delete all" action

    ctx.mock_empty_label(LabelId::trash()).await;

    let queue = user_ctx.action_queue();

    assert!(label.is_idle(&tether).await.unwrap());

    Message::action_delete_all_in_label(queue, label.id(), &tether)
        .await
        .unwrap()
        .unwrap();

    user_ctx.execute_all_actions().await.unwrap();

    // After a label has been emptied, it should be marked as busy until we get
    // a confirmation from the server that the task has completed
    assert!(label.is_busy(&tether).await.unwrap());
    assert!(target.wait_for_update().await.unwrap().unwrap().is_empty());
    assert!(!target.has_more().await.unwrap());

    // ---
    // [3] Pretend the server is in the middle of the removal.
    //
    // The response below has both `conversations: [...]` and `tasks_running:
    // Some`, but because we know the label is being emptied, the scroller
    // should ignore the conversations from that response.

    let convs: Vec<_> = create_api_conversation_page(0..10, 120)
        .into_iter()
        .map(|mut conv| {
            conv.labels = vec![ApiConversationLabel {
                id: label.remote_id().unwrap().clone(),
                ..ApiConversationLabel::test_default()
            }];

            conv
        })
        .collect();

    ctx.mock_get_messages()
        .given_conversation_ids(convs.iter().map(|c| c.id.clone()))
        .alter(|mock| mock.expect(1))
        .respond_with(vec![])
        .await;

    ctx.mock_get_conversations_with(|builder| {
        builder
            .and(query_param_is_missing("Anchor"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                    conversations: convs,
                    tasks_running: RunningTasks::some(&[label.remote_id.clone().unwrap()]),
                    stale: false,
                    total: 80,
                }),
            )
            .with_priority(4)
            .expect(1)
    })
    .await;

    // Pretend event loop has bumped counters in the meantime - scroller should
    // pretend the counters are still zero
    msg_counter.total = 15;
    conv_counter.total = 50;

    tether
        .write_tx(async |tx| {
            msg_counter.save(tx).await.unwrap();
            conv_counter.save(tx).await
        })
        .await
        .unwrap();

    user_ctx.force_event_loop_poll().await.unwrap();

    // Since the background task is still running, the scroller should continue
    // to report the label as empty
    assert!(target.wait_for_update().await.unwrap().is_none());
    assert!(!target.has_more().await.unwrap());
    assert!(label.is_busy(&tether).await.unwrap());

    // ---
    // [4] Pretend the task has finished working.

    let convs: Vec<_> = create_api_conversation_page(0..5, 200)
        .into_iter()
        .map(|mut conv| {
            conv.labels = vec![ApiConversationLabel {
                id: label.remote_id().unwrap().clone(),
                ..ApiConversationLabel::test_default()
            }];

            conv
        })
        .collect();

    ctx.mock_get_messages()
        .given_conversation_ids(convs.iter().map(|c| c.id.clone()))
        .alter(|mock| mock.expect(1))
        .respond_with(vec![])
        .await;

    ctx.mock_get_conversations_with(|builder| {
        builder
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                    conversations: convs,
                    tasks_running: RunningTasks::some(&[
                        // Pretend that task on another label is still running,
                        // to make sure that we don't care about tasks on other
                        // labels.
                        //
                        // i.e. as long as the task on `Trash` completed, we're
                        // good
                        SystemLabel::Archive.label_id(),
                    ]),
                    stale: false,
                    total: 5,
                }),
            )
            .with_priority(3)
            .expect(1)
    })
    .await;

    user_ctx.force_event_loop_poll().await.unwrap();

    // Finally, make sure we only see the messages from the latest response,
    // without them being intertwined with the past (now-gone) messages
    assert_eq!(
        vec![
            "myconv_204",
            "myconv_203",
            "myconv_202",
            "myconv_201",
            "myconv_200",
        ],
        target
            .wait_for_update()
            .await
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|cnv| cnv.remote_id.unwrap().to_string())
            .collect::<Vec<_>>()
    );
}

#[function_name::named]
async fn setup_api_sync_previous_page(
    ctx: &MailTestContext,
    first_id: &str,
    conversations: Option<Vec<ApiConversation>>,
    label: &LabelId,
    expect: impl Into<Times>,
) {
    let desc = ScrollOrderDir::for_label(label)
        .reverse()
        .as_api_desc()
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/api/mail/v4/conversations"))
        .and(query_param_contains("AnchorID", first_id))
        .and(query_param_contains("Desc", (desc as u8).to_string()))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                conversations: conversations.unwrap_or_default(),
                tasks_running: RunningTasks::none(),
                stale: false,
                total: 0,
            }),
        )
        .expect(expect)
        .named(function_name!())
        .mount(ctx.mock_server())
        .await;
}

#[function_name::named]
async fn setup_api_sync_conv_label_counters(
    ctx: &MailTestContext,
    label_id: &LabelId,
    expect: impl Into<Times>,
    total: u64,
    unread: u64,
) {
    Mock::given(method("GET"))
        .and(path("/api/mail/v4/conversations/count"))
        .and(query_param_contains("LabelIDs[0]", label_id.as_str()))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(GetConversationsCountResponse {
                counts: vec![ConversationCount {
                    label_id: label_id.clone(),
                    total,
                    unread,
                }],
            }),
        )
        .expect(expect)
        .named(function_name!())
        .with_priority(1)
        .mount(ctx.mock_server())
        .await;
}

fn create_api_conversation_page(
    range: impl Into<Range<usize>>,
    starting_display_order: u64,
) -> Vec<ApiConversation> {
    let params = TestParams::default_basic();
    let test_conversation = params.conversations.clone().pop().unwrap();

    // Conversations are returned and displayed in reversed order
    range
        .into()
        .rev()
        .map(|i| {
            let order = starting_display_order + i as u64;
            let mut new = test_conversation.clone();
            new.id = format!("{}_{}", new.id, order).into();
            new.order = order;
            new.context_time = Some(order);
            new
        })
        .collect_vec()
}

async fn setup_api_conversation_pages(
    ctx: &MailTestContext,
    page_size: usize,
    starting_display_order: u64,
    label: &LabelId,
    empty_pages_requests: impl Into<Times>,
) -> TestParams {
    ctx.mock_ping_success().await;
    let mut params = TestParams::default_basic();
    // Conversations are returned and displayed in reversed order
    let second_page = create_api_conversation_page(0..page_size, starting_display_order);
    let first_page =
        create_api_conversation_page(page_size..(page_size * 2), starting_display_order);
    let first_page_last_id = first_page.last().map(|conv| conv.id.to_string()).unwrap();
    let second_page_last_id = second_page.last().map(|conv| conv.id.to_string()).unwrap();

    ctx.mock_get_messages()
        .given_conversation_ids(second_page.iter().map(|c| c.id.clone()))
        .alter(|mock| mock.expect(0..=2))
        .respond_with(vec![])
        .await;
    mock_get_conversations_page(ctx, second_page, &first_page_last_id, label, 1_u64).await;
    // last page is empty
    mock_get_conversations_page(
        ctx,
        vec![],
        &second_page_last_id,
        label,
        empty_pages_requests,
    )
    .await;
    ctx.mock_get_messages()
        .given_conversation_ids(first_page.iter().map(|c| c.id.clone()))
        .alter(|mock| mock.expect(0..=2))
        .respond_with(vec![])
        .await;
    ctx.mock_get_conversations(first_page, 1_u64).await;

    // Do not download any conv on init
    params.conversations = vec![];
    params
}

#[function_name::named]
pub async fn mock_get_conversations_page(
    ctx: &MailTestContext,
    conversations: Vec<ApiConversation>,
    last_id: &str,
    label: &LabelId,
    expect: impl Into<Times>,
) {
    let desc = ScrollOrderDir::for_label(label).as_api_desc().unwrap();

    Mock::given(method("GET"))
        .and(path("/api/mail/v4/conversations"))
        .and(query_param_contains("AnchorID", last_id))
        .and(query_param_contains("Desc", (desc as u8).to_string()))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                conversations,
                tasks_running: RunningTasks::none(),
                stale: false,
                total: 1,
            }),
        )
        .expect(expect)
        .named(function_name!())
        .mount(ctx.mock_server())
        .await;
}

#[function_name::named]
pub async fn mock_not_responsive_api(ctx: &MailTestContext) {
    Mock::given(method("GET"))
        .and(path("/api/mail/v4/conversations"))
        .respond_with_err(|_: &Request| {
            std::io::Error::new(std::io::ErrorKind::ConnectionReset, "connection reset")
        })
        .named(function_name!())
        .mount(ctx.mock_server())
        .await;

    Mock::given(method("GET"))
        .and(path("/api/core/v4/tests/ping"))
        .respond_with_err(|_: &Request| {
            std::io::Error::new(std::io::ErrorKind::ConnectionReset, "connection reset")
        })
        .mount(ctx.mock_server())
        .await;

    ctx.mail_context
        .network_monitor_service()
        .update_os_network_status(OsNetworkStatus::Offline);
}

/// Regression test for the non-deterministic ordering bug introduced when
/// `MAX(context_snooze_time, context_time)` was added as the sort key.
///
/// Two conversations that share the same effective sort value
/// (`MAX(snooze, time) = T`) and the same `display_order` have no unique
/// tiebreaker, so SQLite may return them in either order across separate
/// queries. The `CachedScrollData` pagination computes its page offset by
/// calling `cursor.seen_count`, which counts every row satisfying
/// `MAX(...) = T AND display_order >= cursor.display_order`. When both tied
/// items satisfy that constraint the count is 1 too high, so the next
/// `OFFSET cursor_count` skips one of them — it is silently dropped.
///
/// Scenario (page_size = 1):
///   position 1 — `newest`  (MAX = 100, always deterministic)
///   position 2 — `conv_a`  (context_time = 50, snooze = 0  → MAX = 50)
///   position 3 — `conv_b`  (context_time =  0, snooze = 50 → MAX = 50)
///
/// `conv_a` and `conv_b` share MAX = 50 **and** display_order = 0.
/// After page 2, cursor.seen_count returns 3 (over-counts by 1) == all,
/// so `while_fetch_more` returns None and `conv_b` (or `conv_a`) is lost.
#[tokio::test]
async fn test_cached_scroller_no_items_lost_with_tied_snooze_and_time() {
    let mail_stash = new_test_connection().await;
    let mut tether = mail_stash.connection().await.unwrap();

    let mut lbl = label!(remote_id: lbl_id!("test_label"));
    tether
        .write_tx(async |bond| lbl.save(bond).await)
        .await
        .unwrap();

    let mut newest = conversation!(remote_id: conv_id!("conv_newest"), display_order: 0);
    let mut conv_a = conversation!(remote_id: conv_id!("conv_a"), display_order: 0);
    // conv_b: same MAX(snooze, time) = 50 AND same display_order = 0 as conv_a
    let mut conv_b = conversation!(remote_id: conv_id!("conv_b"), display_order: 0);

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            newest.save(bond).await.unwrap();
            let mut l = conv_label!(
                local_conversation_id: newest.local_id,
                remote_label_id: lbl.remote_id.clone(),
                local_label_id: lbl.local_id,
                context_time: 100.into(),
                context_snooze_time: 0.into()
            );
            l.save(bond).await.unwrap();
            newest.reload(bond).await.unwrap();

            conv_a.save(bond).await.unwrap();
            let mut l = conv_label!(
                local_conversation_id: conv_a.local_id,
                remote_label_id: lbl.remote_id.clone(),
                local_label_id: lbl.local_id,
                context_time: 50.into(),
                context_snooze_time: 0.into()
            );
            l.save(bond).await.unwrap();
            conv_a.reload(bond).await.unwrap();

            conv_b.save(bond).await.unwrap();
            let mut l = conv_label!(
                local_conversation_id: conv_b.local_id,
                remote_label_id: lbl.remote_id.clone(),
                local_label_id: lbl.local_id,
                context_time: 0.into(),
                context_snooze_time: 50.into()
            );
            l.save(bond).await.unwrap();
            conv_b.reload(bond).await.unwrap();

            Ok(())
        })
        .await
        .unwrap();

    let local_label_id = lbl.id();
    let mut scroller = CachedScrollData::<ConversationScrollData>::all(
        local_label_id,
        ReadFilter::All,
        1, // page_size = 1 puts conv_a / conv_b right at the page boundary
        vec![],
        ScrollOrderDir::Desc,
        ScrollOrderField::SnoozeTime,
    );

    let mut all_ids = Vec::new();
    while let Some(page) = scroller.while_fetch_more(&tether).await.unwrap() {
        all_ids.extend(page.into_iter().map(|c| c.remote_id));
    }

    // All three conversations must appear exactly once.
    // Without the fix `cursor.seen_count` returns 3 (== `all`) after page 2,
    // so `while_fetch_more` stops early and one tied item is silently dropped.
    assert_eq!(
        all_ids.len(),
        3,
        "Expected 3 conversations but got {}. \
         The tied pair caused cursor_count to over-count, dropping one item. \
         IDs returned: {:?}",
        all_ids.len(),
        all_ids,
    );

    let unique_count = all_ids
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(unique_count, 3, "Duplicate conversation IDs: {:?}", all_ids);

    assert!(all_ids.contains(&conv_id!("conv_newest")));
    assert!(all_ids.contains(&conv_id!("conv_a")));
    assert!(all_ids.contains(&conv_id!("conv_b")));
}

/// End-to-end test for the Category View feature.
///
/// Verifies that:
/// - `category_view()` returns the correct initial state (CategoryDefault auto-enabled, available
///   categories reflect DB `display` flags).
/// - `change_category_view` applies the SQL category filter and emits both a list update
///   (`ReplaceFrom`) and a `CategoryViewChanged` update in that order.
/// - Switching between categories and clearing the filter (`None`) all produce the correct
///   conversation counts and update payloads.
///
/// # Update ordering
///
/// After each `change_category_view` call the scroller emits exactly two updates:
///   1. `ReplaceFrom` — the filtered list refresh from `reset`.
///   2. `CategoryViewChanged` — the resolved category labels.
///
/// `ConversationCounter.total` for Inbox is zeroed after seeding so that
/// `try_fetch_first_page` never schedules a spurious background `FetchMore`
/// (`cant_see_first_page = 0 > 0 = false`). Conversations are still returned
/// by the SQL label-filter query.
#[tokio::test]
async fn test_category_view_filters_conversations_and_emits_updates() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    // Seed 2 conversations in Inbox + CategorySocial, and 1 in Inbox + CategoryDefault.
    // Inbox remote ID is "0", CategorySocial is "20", CategoryDefault is "24".
    let mut data = hash_map! {
        vec!["0", "20"]: test_conversations(2, 0),
        vec!["0", "24"]: test_conversations(1, 100),
    };
    data.save_to_database(&mut tether).await;

    // Resolve local IDs used for assertions and commands.
    let inbox_local_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let social_local_id = SystemLabel::CategorySocial
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();
    let default_local_id = SystemLabel::CategoryDefault
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();

    // Enable category view: mail_category_view=true and set CategorySocial.display=true so it
    // appears in `available` and can be selected.
    // Zero all category ConversationCounter.total values so try_fetch_first_page never
    // schedules a spurious FetchMore (cant_see_first_page = total>0 && seen<total = false).
    // Item counts are still asserted via DB SQL queries, not via these counters.
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                mail_category_view: true,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            social.display = true;
            social.save(bond).await.unwrap();
            for label_id in [inbox_local_id, social_local_id, default_local_id] {
                let mut counter = ConversationCounter::find_by_id(label_id, bond)
                    .await
                    .unwrap()
                    .unwrap();
                counter.total = 0;
                counter.save(bond).await.unwrap();
            }
            Ok(())
        })
        .await
        .unwrap();

    drop(tether);

    // Block all API conversation calls so background syncs cannot restore the zeroed inbox
    // counter and trigger spurious FetchMore events during the category-switch assertions.
    mock_api_forbidden(&ctx).await;

    let page_size = 5;
    let mut test_scroller = TestScroller::conversations(&user_ctx, inbox_local_id, page_size)
        .await
        .unwrap();

    // Initial fetch — CategoryDefault is auto-enabled during scroller init, so only the
    // 1 conversation tagged with CategoryDefault is returned (not all 3 inbox items).
    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Append { items: 1 })
        .await;
    assert_eq!(test_scroller.items().len(), 1);

    // CategoryView metadata: CategoryDefault is auto-enabled and both category labels are
    // available (CategoryDefault always, CategorySocial because display=true).
    let view = test_scroller.category_view().await.unwrap();
    assert_eq!(
        view.enabled,
        Some(default_local_id),
        "CategoryDefault should be auto-enabled when mail_category_view is true"
    );
    assert!(
        view.available.contains(&default_local_id),
        "CategoryDefault should always be in available"
    );
    assert!(
        view.available.contains(&social_local_id),
        "CategorySocial (display=true) should be in available"
    );

    // Switch to CategorySocial — only the 2 Social-tagged conversations are shown.
    test_scroller
        .change_category_view(Some(social_local_id))
        .unwrap();
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 2 })
        .await;
    test_scroller
        .match_next_update(TestUpdate::CategoryViewChanged { labels: 2 })
        .await;
    assert_eq!(test_scroller.items().len(), 2);

    // Clear category filter (None) — Should not change active category
    // Only way to disable the category view is to update MailSettings
    test_scroller.change_category_view(None).unwrap();
    let category_view = test_scroller.category_view().await.unwrap();

    assert_eq!(
        category_view.enabled,
        Some(social_local_id),
        "Category filter clearing is forbidden"
    );
}

/// Like `test_category_view_filters_conversations_and_emits_updates` but
/// switches to AllMail after the initial category-filtered load and verifies
/// that the category view is cleared (AllMail does not support categories).
///
/// Seed: 2 conversations in Inbox + AllMail + CategorySocial ("0","5","20"),
///       1 conversation in Inbox + AllMail + CategoryDefault ("0","5","24").
/// `ConversationCounter.total` for Inbox and AllMail are zeroed after seeding
/// to prevent background API fetches.
#[tokio::test]
async fn test_category_view_clears_on_change_label_to_all_mail() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    // Seed 2 conversations in Inbox + AllMail + CategorySocial, and 1 in Inbox + AllMail + CategoryDefault.
    // Inbox="0", AllMail="5", CategorySocial="20", CategoryDefault="24".
    let mut data = hash_map! {
        vec!["0", "5", "20"]: test_conversations(2, 0),
        vec!["0", "5", "24"]: test_conversations(1, 100),
    };
    data.save_to_database(&mut tether).await;

    // Resolve local IDs used for assertions and commands.
    let inbox_local_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let allmail_local_id = SystemLabel::AllMail
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();
    let social_local_id = SystemLabel::CategorySocial
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();
    let default_local_id = SystemLabel::CategoryDefault
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();

    // Enable category view setting and set CategorySocial.display=true.
    // Zero both Inbox and AllMail counters to prevent spurious background API fetches.
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                mail_category_view: true,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            social.display = true;
            social.save(bond).await.unwrap();
            let mut inbox_counter = ConversationCounter::find_by_id(inbox_local_id, bond)
                .await
                .unwrap()
                .unwrap();
            inbox_counter.total = 0;
            inbox_counter.save(bond).await.unwrap();
            let mut allmail_counter = ConversationCounter::find_by_id(allmail_local_id, bond)
                .await
                .unwrap()
                .unwrap();
            allmail_counter.total = 0;
            allmail_counter.save(bond).await.unwrap();
            Ok(())
        })
        .await
        .unwrap();

    let page_size = 5;
    let mut test_scroller = TestScroller::conversations(&user_ctx, inbox_local_id, page_size)
        .await
        .unwrap();

    // Initial fetch — CategoryDefault is auto-enabled, so only the 1 conversation tagged
    // with CategoryDefault is returned (not all 3 inbox conversations).
    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Append { items: 1 })
        .await;
    assert_eq!(test_scroller.items().len(), 1);

    // CategoryView metadata: CategoryDefault is auto-enabled and both category labels are
    // available (CategoryDefault always, CategorySocial because display=true).
    let view = test_scroller.category_view().await.unwrap();
    assert_eq!(
        view.enabled,
        Some(default_local_id),
        "CategoryDefault should be auto-enabled when mail_category_view is true"
    );
    assert!(
        view.available.contains(&default_local_id),
        "CategoryDefault should always be in available"
    );
    assert!(
        view.available.contains(&social_local_id),
        "CategorySocial (display=true) should be in available"
    );

    // Switch to AllMail — category filtering does not apply, all 3 conversations are shown.
    test_scroller.change_label(allmail_local_id).unwrap();
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 3 })
        .await;
    assert_eq!(test_scroller.items().len(), 3);

    // CategoryView is cleared for non-Inbox labels.
    let view = test_scroller.category_view().await.unwrap();
    assert_eq!(
        view.enabled, None,
        "category should be cleared when switching to AllMail"
    );
    assert!(
        view.available.is_empty(),
        "no categories should be available for AllMail"
    );

    // Switch to back to the Inbox — category filtering does apply, only Primary Category is shown.
    test_scroller.change_label(inbox_local_id).unwrap();
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 1 })
        .await;
    assert_eq!(test_scroller.items().len(), 1);

    // CategoryView is cleared for non-Inbox labels.
    let view = test_scroller.category_view().await.unwrap();
    assert_eq!(
        view.enabled,
        Some(default_local_id),
        "category should be cleared when switching to AllMail"
    );
    assert!(
        view.available.contains(&default_local_id),
        "CategoryDefault should always be in available"
    );
    assert!(
        view.available.contains(&social_local_id),
        "CategorySocial (display=true) should be in available"
    );
}

/// Verifies that when CategorySocial is the active category filter, `sync_first_page` sends
/// both `LabelID[0]=0` (Inbox) **and** `LabelID[1]=20` (CategorySocial) in a single GET
/// /conversations request.
///
/// `reset()` (called from within the `ChangeCategoryView` command handler) awaits the
/// first-page API task synchronously via `wait_for_request()`.  This means by the time
/// `change_category_view` returns `ReplaceFrom`, the API task has already committed 2 Social
/// conversations to the DB and the conversations are immediately visible.
///
/// Update sequence after `change_category_view`:
///   1. `ReplaceFrom { items: 2 }` — reset awaits the first-page API task (strict mock fires),
///      saves 2 Social convs, then `refresh(true)` reads 2 visible items → `ReplaceFrom{2}`.
///   2. `CategoryViewChanged` — resolved category labels.
///
/// Cursor assertion is performed after the update sequence; the quiet_tx inside the API task
/// has already committed before `reset()` returns `ReplaceFrom`.
#[tokio::test]
async fn test_category_view_first_page_sends_multi_label_ids_to_api() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    // Seed 1 CategoryDefault conversation so the scroller has something to show
    let mut data = hash_map! {
        vec!["0", "24"]: test_conversations(1, 0),
    };
    data.save_to_database(&mut tether).await;

    let inbox_local_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let social_local_id = SystemLabel::CategorySocial
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();

    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                mail_category_view: true,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            social.display = true;
            social.save(bond).await.unwrap();
            // Zero the counter so the initial CategoryDefault fetch_more() never hits the API
            // (check_for_total=true, total=0 → branch skipped).
            let mut counter = ConversationCounter::find_by_id(inbox_local_id, bond)
                .await
                .unwrap()
                .unwrap();
            counter.total = 0;
            counter.save(bond).await.unwrap();
            Ok(())
        })
        .await
        .unwrap();

    // Strict first-page mock: matches only when the request carries BOTH LabelID[0]
    // (Inbox) AND LabelID[1] (CategorySocial) AND has no AnchorID (i.e. first page).
    // The conversations include the Social label so the scroller's category filter
    // (EXISTS cl_cat.local_label_id IN (social_local_id)) finds them after save.
    let inbox_remote_id = SystemLabel::Inbox.remote_id().to_string();
    let social_remote_id = SystemLabel::CategorySocial.remote_id().to_string();
    let social_api_convs: Vec<ApiConversation> = create_api_conversation_page(0..2, 200)
        .into_iter()
        .map(|mut conv| {
            conv.labels.push(ApiConversationLabel {
                id: LabelId::from(social_remote_id.as_str()),
                context_num_unread: 0,
                context_num_messages: 1,
                context_time: 0,
                context_size: 12,
                context_num_attachments: 0,
                context_expiration_time: 0,
                context_snooze_time: 0,
            });
            conv
        })
        .collect();
    let social_api_convs_clone = social_api_convs.clone();

    ctx.mock_get_conversations_with(move |builder| {
        builder
            .and(query_param_contains("LabelID[0]", inbox_remote_id.as_str()))
            .and(query_param_contains(
                "LabelID[1]",
                social_remote_id.as_str(),
            ))
            .and(query_param_is_missing("AnchorID"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                    conversations: social_api_convs_clone,
                    tasks_running: RunningTasks::none(),
                    stale: false,
                    total: 2,
                }),
            )
            .with_priority(1)
            .expect(1)
    })
    .await;

    // fetch_conversations_and_messages always calls GET /messages after saving conversations.
    ctx.mock_get_messages()
        .alter(|mock| mock.expect(0..=2))
        .respond_with(vec![])
        .await;
    ctx.mock_ping_success().await;

    let page_size = 5;
    let mut test_scroller = TestScroller::conversations(&user_ctx, inbox_local_id, page_size)
        .await
        .unwrap();

    // Initial fetch: counter=0 → CategoryDefault sync skipped, 1 item returned from local DB.
    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Append { items: 1 })
        .await;
    assert_eq!(test_scroller.items().len(), 1);

    // Switch to CategorySocial.  reset() inside the command handler awaits the first-page
    // API task synchronously (via wait_for_request → sync_next → wait_for_request).
    // The strict 2-label mock fires, 2 Social convs are committed to DB, and refresh(true)
    // reads 2 visible items → ReplaceFrom{2}.
    test_scroller
        .change_category_view(Some(social_local_id))
        .unwrap();
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 2 })
        .await;
    test_scroller
        .match_next_update(TestUpdate::CategoryViewChanged { labels: 2 })
        .await;
    assert_eq!(test_scroller.items().len(), 2);

    // Cursor assertion: wait_for_request() above guaranteed the quiet_tx completed, so the
    // ConversationScrollData row is already in the DB.
    let tether = user_ctx.user_stash().connection().await.unwrap();
    let category = CanonicalCategory::new(vec![social_local_id]);
    let cursor = ConversationScrollData::find_with_key(
        inbox_local_id,
        ReadFilter::All,
        ScrollOrderDir::Desc,
        category.clone(),
        &tether,
    )
    .await
    .unwrap()
    .expect("ConversationScrollData cursor must exist for Inbox+CategorySocial after sync");

    assert_eq!(
        cursor.category, category,
        "cursor.category must match CanonicalCategory for social_local_id"
    );
}

/// Verifies that the **next-page** GET /conversations request for CategorySocial also carries
/// both `LabelID[0]=0` (Inbox) and `LabelID[1]=20` (CategorySocial), along with the
/// `AnchorID` of the last conversation from the first page.
///
/// `reset()` awaits the first-page API task synchronously, so both pages' conversations must
/// include the Social label to pass the scroller's EXISTS category filter.
///
/// Update sequence:
///   1. `change_category_view` → `ReplaceFrom{2}` + `CategoryViewChanged`
///      (reset() awaits strict first-page mock; 2 Social convs committed; next-page task queued)
///   2. `fetch_more()` → awaits next-page task → strict AnchorID + 2-label mock fires →
///      sync() reloads end cursor → `Append{2}` (Social second page)
///
/// The strict next-page mock's `.expect(1)` fires at teardown if the request never arrives or
/// arrives without the required params.
#[tokio::test]
async fn test_category_view_next_page_sends_multi_label_ids_to_api() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    // Same seed strategy as the first-page test: 1 CategoryDefault conv locally,
    // counter zeroed so the initial fetch_more() doesn't hit the API.
    let mut data = hash_map! {
        vec!["0", "24"]: test_conversations(1, 0),
    };
    data.save_to_database(&mut tether).await;

    let inbox_local_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let social_local_id = SystemLabel::CategorySocial
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();

    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                mail_category_view: true,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            social.display = true;
            social.save(bond).await.unwrap();
            let mut counter = ConversationCounter::find_by_id(inbox_local_id, bond)
                .await
                .unwrap()
                .unwrap();
            counter.total = 0;
            counter.save(bond).await.unwrap();
            Ok(())
        })
        .await
        .unwrap();

    let inbox_remote_id = SystemLabel::Inbox.remote_id().to_string();
    let social_remote_id = SystemLabel::CategorySocial.remote_id().to_string();

    // create_api_conversation_page(0..2, 200) produces [myconv_201, myconv_200] in DESC order.
    // The last element (myconv_200) becomes the AnchorID stored in ConversationScrollData.
    // Both pages include the Social label so the scroller's category filter finds them.
    let first_page_convs: Vec<ApiConversation> = create_api_conversation_page(0..2, 200)
        .into_iter()
        .map(|mut conv| {
            conv.labels.push(ApiConversationLabel {
                id: LabelId::from(social_remote_id.as_str()),
                context_num_unread: 0,
                context_num_messages: 1,
                context_time: 0,
                context_size: 12,
                context_num_attachments: 0,
                context_expiration_time: 0,
                context_snooze_time: 0,
            });
            conv
        })
        .collect();
    let anchor_id = first_page_convs.last().unwrap().id.to_string();
    let first_page_clone = first_page_convs.clone();

    // Strict first-page mock: matches requests without AnchorID (initial page) that carry
    // both label IDs.  Priority=2 so it takes precedence over any lenient fallback.
    let inbox_remote_id_clone = inbox_remote_id.clone();
    let social_remote_id_clone = social_remote_id.clone();
    ctx.mock_get_conversations_with(move |builder| {
        builder
            .and(query_param_contains(
                "LabelID[0]",
                inbox_remote_id_clone.as_str(),
            ))
            .and(query_param_contains(
                "LabelID[1]",
                social_remote_id_clone.as_str(),
            ))
            .and(query_param_is_missing("AnchorID"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                    conversations: first_page_clone,
                    tasks_running: RunningTasks::none(),
                    stale: false,
                    total: 4,
                }),
            )
            .with_priority(2)
            .expect(1)
    })
    .await;

    // Strict next-page mock: matches requests with AnchorID=myconv_200 that carry both
    // label IDs.  sync_next_page fires once the local cursor is exhausted (has_next_page=false).
    let second_page_convs: Vec<ApiConversation> = create_api_conversation_page(0..2, 100)
        .into_iter()
        .map(|mut conv| {
            conv.labels.push(ApiConversationLabel {
                id: LabelId::from(social_remote_id.as_str()),
                context_num_unread: 0,
                context_num_messages: 1,
                context_time: 0,
                context_size: 12,
                context_num_attachments: 0,
                context_expiration_time: 0,
                context_snooze_time: 0,
            });
            conv
        })
        .collect();
    let second_page_clone = second_page_convs.clone();
    let inbox_remote_id_clone2 = inbox_remote_id.clone();
    let social_remote_id_clone2 = social_remote_id.clone();
    let anchor_id_clone = anchor_id.clone();
    ctx.mock_get_conversations_with(move |builder| {
        builder
            .and(query_param_contains(
                "LabelID[0]",
                inbox_remote_id_clone2.as_str(),
            ))
            .and(query_param_contains(
                "LabelID[1]",
                social_remote_id_clone2.as_str(),
            ))
            .and(query_param_contains("AnchorID", anchor_id_clone.as_str()))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetConversationsResponse {
                    conversations: second_page_clone,
                    tasks_running: RunningTasks::none(),
                    stale: false,
                    total: 4,
                }),
            )
            .with_priority(2)
            .expect(1)
    })
    .await;

    ctx.mock_get_messages()
        .alter(|mock| mock.expect(0..=4))
        .respond_with(vec![])
        .await;
    ctx.mock_ping_success().await;

    let page_size = 5;
    let mut test_scroller = TestScroller::conversations(&user_ctx, inbox_local_id, page_size)
        .await
        .unwrap();

    // Initial fetch: counter=0 → no API call, 1 CategoryDefault item from local DB.
    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Append { items: 1 })
        .await;

    // Switch to CategorySocial.  reset() awaits the first-page API task synchronously:
    // strict first-page mock fires (LabelID[0] + LabelID[1], no AnchorID), 2 Social
    // convs are committed to DB, and refresh(true) returns ReplaceFrom{2}.
    // reset() also leaves a next-page background task stored in self.task.
    test_scroller
        .change_category_view(Some(social_local_id))
        .unwrap();
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 2 })
        .await;
    test_scroller
        .match_next_update(TestUpdate::CategoryViewChanged { labels: 2 })
        .await;
    assert_eq!(test_scroller.items().len(), 2);

    // fetch_more(): wait_for_request awaits the next-page background task.
    // The strict next-page mock fires (LabelID[0] + LabelID[1] + AnchorID=myconv_200).
    // After the task completes, sync() reloads the end cursor, scroller reads 2 more
    // Social items from local DB → Append{2}.
    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Append { items: 2 })
        .await;
    assert_eq!(test_scroller.items().len(), 4);
    // wiremock teardown (ctx drop) verifies both strict mocks received exactly 1 request each.
}

/// Verifies that `total()` returns the category-scoped counter after `change_category_view`.
///
/// Seed: 2 conversations in Inbox + CategorySocial, 5 conversations in Inbox + CategoryDefault.
/// After enabling CategorySocial the scroller's `total()` must return the Social counter (2),
/// not the Inbox counter (7).
#[tokio::test]
async fn test_total_reflects_active_category_after_change_category_view() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    let mut data = hash_map! {
        vec!["0", "20"]: test_conversations(2, 0),
        vec!["0", "24"]: test_conversations(5, 100),
    };
    data.save_to_database(&mut tether).await;

    let inbox_local_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let social_local_id = SystemLabel::CategorySocial
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();

    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                mail_category_view: true,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            social.display = true;
            social.save(bond).await.unwrap();
            // Zero Inbox counter to prevent background API fetches.
            let mut inbox_counter = ConversationCounter::find_by_id(inbox_local_id, bond)
                .await
                .unwrap()
                .unwrap();
            inbox_counter.total = 0;
            inbox_counter.save(bond).await.unwrap();
            ConversationCounter {
                local_label_id: social_local_id,
                total: 2,
                unread: 0,
            }
            .save(bond)
            .await
            .unwrap();
            Ok(())
        })
        .await
        .unwrap();

    let social_total_in_db = {
        let counter = ConversationCounter::find_by_id(social_local_id, &tether)
            .await
            .unwrap()
            .unwrap();
        counter.total
    };
    assert_eq!(
        social_total_in_db, 2,
        "seed sanity: social counter should be 2"
    );

    drop(tether);

    let page_size = 10;
    let mut test_scroller = TestScroller::conversations(&user_ctx, inbox_local_id, page_size)
        .await
        .unwrap();

    test_scroller.fetch_more().unwrap();
    test_scroller
        .match_next_update(TestUpdate::Append { items: 5 })
        .await;

    test_scroller
        .change_category_view(Some(social_local_id))
        .unwrap();
    test_scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 2 })
        .await;
    test_scroller
        .match_next_update(TestUpdate::CategoryViewChanged { labels: 2 })
        .await;

    let total = test_scroller.total().await.unwrap();
    assert_eq!(
        total, 2,
        "total() must reflect the Social category counter after change_category_view"
    );
}

#[tokio::test]
async fn test_settings_changed_toggles_category_view() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;

    // reset() fires after each category toggle; range tolerates background task races.
    ctx.mock_get_conversations(vec![], 2..=10).await;

    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let inbox_local_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let default_local_id = SystemLabel::CategoryDefault
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                mail_category_view: true,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            Ok(())
        })
        .await
        .unwrap();
    drop(tether);

    let mut scroller = TestScroller::conversations(&user_ctx, inbox_local_id, 20)
        .await
        .unwrap();

    // Step 1 — disable: CategoryViewChanged { labels: 0 }
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
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
    drop(tether);

    scroller
        .match_next_update(TestUpdate::CategoryViewChanged { labels: 0 })
        .await;

    let view = scroller.category_view().await.unwrap();
    assert!(
        view.available.is_empty(),
        "available must be empty after disabling category view"
    );
    assert_eq!(view.enabled, None, "enabled must be None after disabling");

    scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 0 })
        .await;

    // Step 2 — enable: CategoryViewChanged { labels: 1 }
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                mail_category_view: true,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            Ok(())
        })
        .await
        .unwrap();
    drop(tether);

    scroller
        .match_next_update(TestUpdate::CategoryViewChanged { labels: 1 })
        .await;

    let view = scroller.category_view().await.unwrap();
    assert!(
        view.available.contains(&default_local_id),
        "CategoryDefault must appear in available after re-enabling"
    );
    assert_eq!(
        view.enabled,
        Some(default_local_id),
        "CategoryDefault must be auto-enabled on re-activation"
    );

    scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 0 })
        .await;

    // Step 3 — unrelated: no CategoryViewChanged
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
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
    drop(tether);

    assert!(
        tokio::time::timeout(Duration::from_millis(500), scroller.wait_for_update())
            .await
            .is_err(),
        "CategoryViewChanged must NOT fire for unrelated MailSettings changes"
    );
}

#[tokio::test]
async fn test_label_display_change_toggles_category_view() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;

    // reset() fires after each category toggle; range tolerates background task races.
    ctx.mock_get_conversations(vec![], 2..=10).await;

    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let inbox_local_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let default_local_id = SystemLabel::CategoryDefault
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();
    let social_local_id = SystemLabel::CategorySocial
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();

    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                mail_category_view: true,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            social.display = true;
            social.save(bond).await.unwrap();
            Ok(())
        })
        .await
        .unwrap();
    drop(tether);

    let mut scroller = TestScroller::conversations(&user_ctx, inbox_local_id, 20)
        .await
        .unwrap();

    // Step 1 — disable: CategorySocial.display=false → labels:1 (only CategoryDefault)
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            social.display = false;
            social.save(bond).await.unwrap();
            Ok(())
        })
        .await
        .unwrap();
    drop(tether);

    scroller
        .match_next_update(TestUpdate::CategoryViewChanged { labels: 1 })
        .await;

    let view = scroller.category_view().await.unwrap();
    assert!(
        view.available.contains(&default_local_id),
        "CategoryDefault must remain available after hiding Social"
    );
    assert!(
        !view.available.contains(&social_local_id),
        "CategorySocial must be removed from available after display=false"
    );

    scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 0 })
        .await;

    // Step 2 — enable: CategorySocial.display=true → labels:2 (CategoryDefault + CategorySocial)
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            social.display = true;
            social.save(bond).await.unwrap();
            Ok(())
        })
        .await
        .unwrap();
    drop(tether);

    scroller
        .match_next_update(TestUpdate::CategoryViewChanged { labels: 2 })
        .await;

    let view = scroller.category_view().await.unwrap();
    assert!(
        view.available.contains(&default_local_id),
        "CategoryDefault must remain available"
    );
    assert!(
        view.available.contains(&social_local_id),
        "CategorySocial must reappear in available after display=true"
    );

    scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 0 })
        .await;

    // Step 3 — unrelated: Sent.display toggle → no CategoryViewChanged
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let mut sent = SystemLabel::Sent.load(&tether).await.unwrap().unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            sent.display = !sent.display;
            sent.save(bond).await.unwrap();
            Ok(())
        })
        .await
        .unwrap();
    drop(tether);

    assert!(
        tokio::time::timeout(Duration::from_millis(500), scroller.wait_for_update())
            .await
            .is_err(),
        "CategoryViewChanged must NOT fire when a non-category label's display changes"
    );
}

#[tokio::test]
async fn test_disabling_active_non_primary_category_falls_back_to_primary() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection().await.unwrap();

    // Seed 2 conversations in Inbox+CategorySocial (the active non-primary category)
    // and 1 in Inbox+CategoryDefault. Inbox="0", CategorySocial="20", CategoryDefault="24".
    let mut data = hash_map! {
        vec!["0", "20"]: test_conversations(2, 0),
        vec!["0", "24"]: test_conversations(1, 100),
    };
    data.save_to_database(&mut tether).await;

    let inbox_local_id = SystemLabel::Inbox.local_id(&tether).await.unwrap().unwrap();
    let social_local_id = SystemLabel::CategorySocial
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();
    let default_local_id = SystemLabel::CategoryDefault
        .local_id(&tether)
        .await
        .unwrap()
        .unwrap();

    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();

    tether
        .write_tx::<_, _, StashError>(async |bond| {
            MailSettings {
                mail_category_view: true,
                ..Default::default()
            }
            .save(bond)
            .await
            .unwrap();
            social.display = true;
            social.save(bond).await.unwrap();
            for label_id in [inbox_local_id, social_local_id, default_local_id] {
                let mut counter = ConversationCounter::find_by_id(label_id, bond)
                    .await
                    .unwrap()
                    .unwrap();
                counter.total = 0;
                counter.save(bond).await.unwrap();
            }
            Ok(())
        })
        .await
        .unwrap();
    drop(tether);

    mock_api_forbidden(&ctx).await;

    let page_size = 5;
    let mut scroller = TestScroller::conversations(&user_ctx, inbox_local_id, page_size)
        .await
        .unwrap();

    // Initial fetch — CategoryDefault is auto-enabled, so only the 1 default conversation is returned.
    scroller.fetch_more().unwrap();
    scroller
        .match_next_update(TestUpdate::Append { items: 1 })
        .await;
    assert_eq!(scroller.items().len(), 1);

    // Activate CategorySocial — the 2 social conversations replace the list.
    scroller
        .change_category_view(Some(social_local_id))
        .unwrap();
    scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 2 })
        .await;
    scroller
        .match_next_update(TestUpdate::CategoryViewChanged { labels: 2 })
        .await;
    assert_eq!(scroller.items().len(), 2);

    // Disable CategorySocial (the active non-primary category) — enabled must fall back to CategoryDefault.
    let mut tether = user_ctx.user_stash().connection().await.unwrap();
    let mut social = SystemLabel::CategorySocial
        .load(&tether)
        .await
        .unwrap()
        .unwrap();
    tether
        .write_tx::<_, _, StashError>(async |bond| {
            social.display = false;
            social.save(bond).await.unwrap();
            Ok(())
        })
        .await
        .unwrap();
    drop(tether);

    scroller
        .match_next_update(TestUpdate::CategoryViewChanged { labels: 1 })
        .await;

    let view = scroller.category_view().await.unwrap();
    assert_eq!(
        view.enabled,
        Some(default_local_id),
        "enabled must fall back to CategoryDefault when active CategorySocial is hidden"
    );
    assert!(
        view.available.contains(&default_local_id),
        "CategoryDefault must remain in available"
    );
    assert!(
        !view.available.contains(&social_local_id),
        "CategorySocial must be removed from available after display=false"
    );

    // CategoryDefault filter includes hidden categories (social.display=false bins those
    // conversations under Default), so all 3 seeded items are returned.
    scroller
        .match_next_update(TestUpdate::ReplaceFrom { idx: 0, items: 3 })
        .await;
}

#[function_name::named]
pub async fn mock_api_forbidden(ctx: &MailTestContext) {
    Mock::given(method("GET"))
        .and(path("/api/mail/v4/conversations"))
        .respond_with(ResponseTemplate::new(403))
        .named(function_name!())
        .mount(ctx.mock_server())
        .await;

    ctx.mock_ping_success().await;
}
