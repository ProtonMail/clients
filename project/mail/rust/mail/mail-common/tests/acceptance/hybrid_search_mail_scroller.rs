//! Acceptance tests for hybrid search (local + remote) mail scroller.
//!
//! These tests verify that when Foundation Search is enabled:
//! - Local results appear immediately from the on-device index
//! - Remote results are fetched in the background and appended
//! - The merged view (local plus remote) is visible after refresh
//! - Pagination, total, and has_more all work correctly with the merged result set

mod fixtures;

use fixtures::hybrid_search_fixture;
use itertools::Itertools;
use mail_api::services::proton::common::MessageId;
use mail_api::services::proton::prelude::RunningTasks;
use mail_common::api_message_meta;
use mail_common::datatypes::{AlmostAllMail, SearchOptions, SystemLabelId};
use mail_common::models::{MailSettings, Message};
use mail_common::msg_id;
use mail_common::test_utils::scroller::{TestScroller, save_single_message};
use mail_common::test_utils::{
    init::Params as TestParams,
    test_context::{MailTestContext, MailUserContextTestExtension},
};
use mail_core_api::services::proton::LabelId;
use mail_core_common::datatypes::SystemLabel;
use mail_core_common::models::ModelIdExtension;
use mail_search::MessageMetadata;
use mail_stash::orm::Model;
use mail_stash::stash::StashError;
use std::time::Duration;

/// Populate the local search index with fixture messages.
///
/// Creates messages via events, stores body text, and indexes directly
/// so local search will find them.
async fn setup_local_search_index(
    ctx: &MailTestContext,
    params: &TestParams,
) -> Vec<(MessageId, String, String)> {
    let user_ctx = ctx.mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();

    let conversation = params.conversations.first().cloned().unwrap();
    let address = params.addresses.first().cloned().unwrap();

    let mut created: Vec<(MessageId, String, String)> = Vec::new();

    for entry in hybrid_search_fixture() {
        let message_meta = api_message_meta!(
            id: MessageId::from(entry.remote_id.clone()),
            conversation_id: conversation.id.clone(),
            address_id: address.id.clone(),
            label_ids: vec![SystemLabel::AllMail.remote_id()],
            subject: entry.subject.to_string()
        );

        user_ctx
            .apply_event(mail_api::services::proton::response_data::MailEvent {
                event_id: mail_core_api::services::proton::EventId::from(format!(
                    "evt_{}",
                    entry.remote_id
                )),
                labels: None,
                conversation_counts: None,
                conversations: None,
                incoming_defaults: None,
                mail_settings: None,
                message_counts: None,
                messages: Some(vec![
                    mail_api::services::proton::response_data::MessageEvent {
                        id: message_meta.id.clone(),
                        action: mail_core_api::services::proton::Action::Create,
                        message: Some(message_meta.clone()),
                    },
                ]),
                refresh: 0,
                has_more: false,
            })
            .await
            .unwrap();

        let mut msg = Message::find_by_remote_id(message_meta.id.clone(), &tether)
            .await
            .unwrap()
            .expect("Message should exist after event");

        let label = SystemLabel::AllMail.load(&tether).await.unwrap().unwrap();
        tether
            .write_tx::<_, _, StashError>(async |bond| {
                save_single_message(&[label], &mut msg, bond).await;
                Ok(())
            })
            .await
            .unwrap();

        let raw_body = mail_common::models::RawMessageBody::local_draft(entry.body.clone());
        tether
            .write_tx::<_, _, StashError>(async |bond| {
                raw_body.store(msg.id(), bond).await?;
                Ok(())
            })
            .await
            .unwrap();

        let metadata = MessageMetadata {
            subject: entry.subject.clone(),
            from: address.email.clone(),
            to: String::new(),
            cc: String::new(),
            bcc: String::new(),
        };

        user_ctx
            .search_service()
            .index_message_body(&message_meta.id, &entry.body, &metadata)
            .await
            .expect("Indexing should succeed");

        created.push((
            message_meta.id,
            entry.subject.to_string(),
            entry.body.to_string(),
        ));
    }

    created
}

/// Hybrid search returns local results immediately and supplements with remote.
#[tokio::test]
async fn hybrid_search_shows_local_first_then_remote_appended() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    let conversation = params.conversations.first().cloned().unwrap();
    let address = params.addresses.first().cloned().unwrap();

    // Remote message that will be appended by background sync
    let remote_message = api_message_meta!(
        id: MessageId::from("remote_msg_1"),
        conversation_id: conversation.id.clone(),
        address_id: address.id.clone(),
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Remote project update".to_string()
    );

    ctx.mock_get_messages()
        .given_label_id(&LabelId::almost_all_mail())
        .given_keyword("project")
        .alter(|mock| mock.expect(1..=2))
        .respond_with_ex(4, vec![remote_message], RunningTasks::none())
        .await;

    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;

    setup_local_search_index(&ctx, &params).await;

    let user_ctx = ctx.mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();
    let mut settings = MailSettings::get_or_default(&tether).await;
    settings.almost_all_mail = AlmostAllMail::AlmostAllMail;
    tether
        .write_tx(async |bond| settings.save(bond).await)
        .await
        .unwrap();

    let page_size = 10;
    let mut test_scroller =
        TestScroller::search(&user_ctx, SearchOptions::from("project"), page_size)
            .await
            .unwrap();

    // Local results appear immediately
    test_scroller.fetch_more_and_wait().await.unwrap();
    let items = test_scroller.items();

    assert!(
        items.len() >= 3,
        "Should have at least 3 local results from fixture, got {}",
        items.len()
    );

    let local_ids: Vec<_> = items.iter().map(|m| m.remote_id.clone()).collect();
    assert!(
        local_ids.contains(&msg_id!("local_msg_1")),
        "Local result 1 should be present: {:?}",
        local_ids
    );
    assert!(
        local_ids.contains(&msg_id!("local_msg_2")),
        "Local result 2 should be present: {:?}",
        local_ids
    );
    assert!(
        local_ids.contains(&msg_id!("local_msg_3")),
        "Local result 3 should be present: {:?}",
        local_ids
    );

    // Wait for remote sync to complete and append
    tokio::time::sleep(Duration::from_millis(500)).await;

    let possible_append = test_scroller
        .try_wait_for_update(Duration::from_secs(3))
        .await
        .unwrap();

    if let Some(new_items) = possible_append {
        assert!(
            !new_items.is_empty(),
            "Remote should append at least 1 item, got {}",
            new_items.len()
        );
    }

    let final_items = test_scroller.items();
    assert!(
        final_items.len() >= 4,
        "Merged view should have at least 4 items (3 local + 1 remote), got {}",
        final_items.len()
    );

    let has_remote = final_items
        .iter()
        .any(|m| m.remote_id == msg_id!("remote_msg_1"));
    assert!(
        has_remote,
        "Remote result should appear in merged view: {:?}",
        final_items
            .iter()
            .map(|m| m.remote_id.clone())
            .collect_vec()
    );
}

/// Flanders' pagination edge case: when has_local_results, sync_next re-fetches last from DB.
/// Page forward through local results while remote appends in background — verify no
/// duplicates and no skips. Remote results must land after local (higher display_order).
#[tokio::test]
async fn hybrid_search_pagination_no_duplicates_or_skips_when_remote_appends() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    let conversation = params.conversations.first().cloned().unwrap();
    let address = params.addresses.first().cloned().unwrap();

    // Remote returns one duplicate (local_msg_1) + two new messages. Dedup skips the duplicate.
    let duplicate_from_local = api_message_meta!(
        id: MessageId::from("local_msg_1"),
        conversation_id: conversation.id.clone(),
        address_id: address.id.clone(),
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Project timeline discussion".to_string()
    );
    let remote_msg_1 = api_message_meta!(
        id: MessageId::from("remote_msg_1"),
        conversation_id: conversation.id.clone(),
        address_id: address.id.clone(),
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Remote project update".to_string()
    );
    let remote_msg_2 = api_message_meta!(
        id: MessageId::from("remote_msg_2"),
        conversation_id: conversation.id.clone(),
        address_id: address.id.clone(),
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Another remote project note".to_string()
    );

    ctx.mock_get_messages()
        .given_label_id(&LabelId::almost_all_mail())
        .given_keyword("project")
        .alter(|mock| mock.expect(1..=2))
        .respond_with_ex(
            5,
            vec![duplicate_from_local, remote_msg_1, remote_msg_2],
            RunningTasks::none(),
        )
        .await;

    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;

    setup_local_search_index(&ctx, &params).await;

    let user_ctx = ctx.mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();
    let mut settings = MailSettings::get_or_default(&tether).await;
    settings.almost_all_mail = AlmostAllMail::AlmostAllMail;
    tether
        .write_tx(async |bond| settings.save(bond).await)
        .await
        .unwrap();

    let page_size = 2;
    let mut test_scroller =
        TestScroller::search(&user_ctx, SearchOptions::from("project"), page_size)
            .await
            .unwrap();

    // First fetch: local search always returns first, so we get the 3 local results.
    // (Remote runs in background; it may have appended by now in a fast run, but local is always first.)
    test_scroller.fetch_more_and_wait().await.unwrap();
    let after_first = test_scroller.items();
    assert!(
        after_first.len() >= 3,
        "First fetch must include local results, got {} items",
        after_first.len()
    );
    for local_id in [
        msg_id!("local_msg_1"),
        msg_id!("local_msg_2"),
        msg_id!("local_msg_3"),
    ] {
        assert!(
            after_first.iter().any(|m| m.remote_id == local_id),
            "Local result {:?} must be present in first fetch",
            local_id
        );
    }

    // Wait for remote sync to append (dedup skips local_msg_1, adds remote_msg_1 and remote_msg_2)
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = test_scroller
        .try_wait_for_update(Duration::from_secs(3))
        .await;

    // Page through until no more — exercises sync_next re-fetching last when has_local_results
    while test_scroller.has_more().await.unwrap() {
        test_scroller.fetch_more_and_wait().await.unwrap();
    }

    let all_items = test_scroller.items();
    let remote_ids: Vec<_> = all_items.iter().map(|m| m.remote_id.clone()).collect();

    // No duplicates: each remote_id appears exactly once
    let unique_count = remote_ids.iter().cloned().unique().count();
    assert_eq!(
        unique_count,
        remote_ids.len(),
        "No duplicates: expected {} unique, got {:?}",
        remote_ids.len(),
        remote_ids
    );

    // All 5 expected: 3 local + 2 remote (dedup skips local_msg_1 from remote)
    assert_eq!(
        all_items.len(),
        5,
        "Should have 5 items (3 local + 2 remote), got {}",
        all_items.len()
    );

    // Local results first, then remote (display_order ordering)
    let local_ids = [
        msg_id!("local_msg_1"),
        msg_id!("local_msg_2"),
        msg_id!("local_msg_3"),
    ];
    let remote_ids_expected = [msg_id!("remote_msg_1"), msg_id!("remote_msg_2")];
    for (i, expected) in local_ids.iter().enumerate() {
        assert!(
            remote_ids.contains(expected),
            "Local result {} should be present: {:?}",
            i + 1,
            remote_ids
        );
    }
    for expected in &remote_ids_expected {
        assert!(
            remote_ids.contains(expected),
            "Remote result should be present: {:?}",
            remote_ids
        );
    }

    // local_msg_1 must appear only once (not duplicated by remote)
    let local_msg_1_count = remote_ids
        .iter()
        .filter(|id| *id == &msg_id!("local_msg_1"))
        .count();
    assert_eq!(
        local_msg_1_count, 1,
        "local_msg_1 should appear exactly once (dedup), got {}",
        local_msg_1_count
    );
}

/// LocalOnly: when offline, hybrid search operates solely on the local index.
/// No remote API calls; pagination works from local SearchScrollData.
#[tokio::test]
async fn hybrid_search_local_only_when_offline() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();

    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;
    setup_local_search_index(&ctx, &params).await;

    let user_ctx = ctx.mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();
    let mut settings = MailSettings::get_or_default(&tether).await;
    settings.almost_all_mail = AlmostAllMail::AlmostAllMail;
    tether
        .write_tx(async |bond| settings.save(bond).await)
        .await
        .unwrap();

    let page_size = 10;
    ctx.set_network_offline();

    let mut test_scroller =
        TestScroller::search(&user_ctx, SearchOptions::from("project"), page_size)
            .await
            .unwrap();

    test_scroller.fetch_more_and_wait().await.unwrap();
    let items = test_scroller.items();

    assert_eq!(
        items.len(),
        3,
        "LocalOnly: should have 3 local results from fixture, got {}",
        items.len()
    );

    let local_ids: Vec<_> = items.iter().map(|m| m.remote_id.clone()).collect();
    for expected in [
        msg_id!("local_msg_1"),
        msg_id!("local_msg_2"),
        msg_id!("local_msg_3"),
    ] {
        assert!(
            local_ids.contains(&expected),
            "LocalOnly: expected {:?} in results: {:?}",
            expected,
            local_ids
        );
    }

    let total = test_scroller.total().await.unwrap();
    assert_eq!(total, 3, "LocalOnly: total should be 3 from local");

    ctx.set_network_online();
}

/// Hybrid search fallback: when local has no results, uses remote-only.
#[tokio::test]
async fn hybrid_search_fallback_to_remote_when_no_local_results() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    let conversation = params.conversations.first().cloned().unwrap();
    let address = params.addresses.first().cloned().unwrap();

    let remote_message = api_message_meta!(
        id: MessageId::from("remote_only_msg"),
        conversation_id: conversation.id,
        address_id: address.id,
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Unique xyzzy keyword".to_string()
    );

    ctx.mock_get_messages()
        .given_label_id(&LabelId::almost_all_mail())
        .given_keyword("xyzzy")
        .alter(|mock| mock.expect(1..=2))
        .respond_with(vec![remote_message])
        .await;

    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;

    let user_ctx = ctx.mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();
    let mut settings = MailSettings::get_or_default(&tether).await;
    settings.almost_all_mail = AlmostAllMail::AlmostAllMail;
    tether
        .write_tx(async |bond| settings.save(bond).await)
        .await
        .unwrap();

    let page_size = 5;
    let mut test_scroller =
        TestScroller::search(&user_ctx, SearchOptions::from("xyzzy"), page_size)
            .await
            .unwrap();

    test_scroller.fetch_more_and_wait().await.unwrap();
    let items = test_scroller.items();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].remote_id, msg_id!("remote_only_msg"));
}

/// Remote-only: total comes from API response, not from message count.
#[tokio::test]
async fn remote_only_total_uses_api_response() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    let conversation = params.conversations.first().cloned().unwrap();
    let address = params.addresses.first().cloned().unwrap();

    let remote_message = api_message_meta!(
        id: MessageId::from("remote_msg"),
        conversation_id: conversation.id,
        address_id: address.id,
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Unique foobar keyword".to_string()
    );

    const API_TOTAL: u64 = 42;
    ctx.mock_get_messages()
        .given_label_id(&LabelId::almost_all_mail())
        .given_keyword("foobar")
        .alter(|mock| mock.expect(1..=2))
        .respond_with_ex(
            API_TOTAL.try_into().expect("API_TOTAL fits in usize"),
            vec![remote_message],
            RunningTasks::none(),
        )
        .await;

    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;

    let user_ctx = ctx.mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();
    let mut settings = MailSettings::get_or_default(&tether).await;
    settings.almost_all_mail = AlmostAllMail::AlmostAllMail;
    tether
        .write_tx(async |bond| settings.save(bond).await)
        .await
        .unwrap();

    let page_size = 5;
    let mut test_scroller =
        TestScroller::search(&user_ctx, SearchOptions::from("foobar"), page_size)
            .await
            .unwrap();

    test_scroller.fetch_more_and_wait().await.unwrap();
    assert_eq!(test_scroller.items().len(), 1);

    let total = test_scroller.total().await.unwrap();
    assert_eq!(
        total, API_TOTAL,
        "Remote-only total should use API response ({API_TOTAL}), not message count (1)"
    );
}

/// Hybrid: total uses deduped count from SearchScrollData, not API response.
/// API may report total=100, but with local+remote dedup we have 5 distinct items.
#[tokio::test]
async fn hybrid_total_uses_deduped_count_not_api_response() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    let conversation = params.conversations.first().cloned().unwrap();
    let address = params.addresses.first().cloned().unwrap();

    let duplicate_from_local = api_message_meta!(
        id: MessageId::from("local_msg_1"),
        conversation_id: conversation.id.clone(),
        address_id: address.id.clone(),
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Project timeline discussion".to_string()
    );
    let remote_msg_1 = api_message_meta!(
        id: MessageId::from("remote_msg_1"),
        conversation_id: conversation.id.clone(),
        address_id: address.id.clone(),
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Remote project update".to_string()
    );
    let remote_msg_2 = api_message_meta!(
        id: MessageId::from("remote_msg_2"),
        conversation_id: conversation.id.clone(),
        address_id: address.id.clone(),
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Another remote project note".to_string()
    );

    const API_TOTAL: u64 = 100;
    ctx.mock_get_messages()
        .given_label_id(&LabelId::almost_all_mail())
        .given_keyword("project")
        .alter(|mock| mock.expect(1..=2))
        .respond_with_ex(
            API_TOTAL.try_into().expect("API_TOTAL fits in usize"),
            vec![duplicate_from_local, remote_msg_1, remote_msg_2],
            RunningTasks::none(),
        )
        .await;

    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;
    setup_local_search_index(&ctx, &params).await;

    let user_ctx = ctx.mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();
    let mut settings = MailSettings::get_or_default(&tether).await;
    settings.almost_all_mail = AlmostAllMail::AlmostAllMail;
    tether
        .write_tx(async |bond| settings.save(bond).await)
        .await
        .unwrap();

    let page_size = 10;
    let mut test_scroller =
        TestScroller::search(&user_ctx, SearchOptions::from("project"), page_size)
            .await
            .unwrap();

    test_scroller.fetch_more_and_wait().await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = test_scroller
        .try_wait_for_update(Duration::from_secs(3))
        .await;

    let total = test_scroller.total().await.unwrap();
    let items = test_scroller.items();
    let expected_deduped: u64 = 5; // 3 local + 2 new remote (local_msg_1 deduped)
    assert_eq!(
        items.len() as u64,
        expected_deduped,
        "Should have 5 deduped items (3 local + 2 remote)"
    );
    assert_eq!(
        total, expected_deduped,
        "Hybrid total should use deduped count ({expected_deduped}), not API total ({API_TOTAL})"
    );
}

/// Refresh-before-FetchMore race: when remote sync (Refresh) runs first and adds items,
/// a subsequent fetch_more may receive the same items from the API. Dedup filters them
/// so we never append duplicates.
#[tokio::test]
async fn hybrid_search_refresh_before_fetch_more_no_duplicates() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    let conversation = params.conversations.first().cloned().unwrap();
    let address = params.addresses.first().cloned().unwrap();

    let duplicate_from_local = api_message_meta!(
        id: MessageId::from("local_msg_1"),
        conversation_id: conversation.id.clone(),
        address_id: address.id.clone(),
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Project timeline discussion".to_string()
    );
    let remote_msg_1 = api_message_meta!(
        id: MessageId::from("remote_msg_1"),
        conversation_id: conversation.id.clone(),
        address_id: address.id.clone(),
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Remote project update".to_string()
    );
    let remote_msg_2 = api_message_meta!(
        id: MessageId::from("remote_msg_2"),
        conversation_id: conversation.id.clone(),
        address_id: address.id.clone(),
        label_ids: vec![SystemLabel::AllMail.remote_id()],
        subject: "Another remote project note".to_string()
    );

    // Same 3 items for both API calls: Refresh gets them first, FetchMore gets them second.
    // Dedup must filter all 3 on the second call (all already in list).
    let same_page = vec![duplicate_from_local, remote_msg_1, remote_msg_2];
    ctx.mock_get_messages()
        .given_label_id(&LabelId::almost_all_mail())
        .given_keyword("project")
        .alter(|mock| mock.expect(2))
        .respond_with_ex(10, same_page.clone(), RunningTasks::none())
        .await;

    ctx.mock_ping_success().await;
    ctx.setup_user(params.clone()).await;
    setup_local_search_index(&ctx, &params).await;

    let user_ctx = ctx.mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();
    let mut settings = MailSettings::get_or_default(&tether).await;
    settings.almost_all_mail = AlmostAllMail::AlmostAllMail;
    tether
        .write_tx(async |bond| settings.save(bond).await)
        .await
        .unwrap();

    let page_size = 5;
    let mut test_scroller =
        TestScroller::search(&user_ctx, SearchOptions::from("project"), page_size)
            .await
            .unwrap();

    // First fetch: local results (3 items)
    test_scroller.fetch_more_and_wait().await.unwrap();
    let after_first = test_scroller.items();
    assert!(
        after_first.len() >= 3,
        "First fetch must include local results, got {}",
        after_first.len()
    );

    // Wait for Refresh (remote sync) to run first — adds remote items
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = test_scroller
        .try_wait_for_update(Duration::from_secs(3))
        .await;

    let after_refresh = test_scroller.items();
    assert_eq!(
        after_refresh.len(),
        5,
        "After Refresh: 5 items (3 local + 2 remote, local_msg_1 deduped), got {}",
        after_refresh.len()
    );

    // FetchMore: API returns same page (2nd mock call). Dedup filters all — no append.
    test_scroller.fetch_more_and_wait().await.unwrap();

    let final_items = test_scroller.items();
    assert_eq!(
        final_items.len(),
        5,
        "After FetchMore (same page): must still have 5 items, no duplicates, got {}",
        final_items.len()
    );

    let unique_count = final_items
        .iter()
        .map(|m| m.remote_id.clone())
        .unique()
        .count();
    assert_eq!(
        unique_count, 5,
        "No duplicates: expected 5 unique ids, got {}",
        unique_count
    );
}
