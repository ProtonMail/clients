use core_event_loop::EventId as CoreEventId;
use mail_action_queue::queue::ActionError as QueueActionError;
use mail_common::MailUserContext;
use mail_common::actions::MailActionError;
use mail_common::models::LabelWithCounters;
use mail_common::test_utils::test_context::{MailTestContext, MailUserContextTestExtension};
use mail_core_api::services::proton::EventId;
use mail_core_common::datatypes::SystemLabel;
use mail_core_common::event_loop::event_store::{MAIL_EVENT_TYPE_ID, store_event_id};
use mail_core_common::models::{Label, LabelError, ModelExtension};
use mail_stash::orm::Model;
use mail_stash::stash::{StashError, Tether};
use std::sync::Arc;
use wiremock::ResponseTemplate;

/// The latest mail event id stored in the DB — this is what the action must send
/// to the server. It is deliberately different from the label's unseen marker so a
/// test fails if the wrong id is ever wired into the request.
const STORED_EVENT_ID: &str = "evt-001";
/// The marker carried by a category label that has an unseen message.
const UNSEEN_MARKER: &str = "unseen-badge";

async fn setup() -> (MailTestContext, Arc<MailUserContext>) {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    (ctx, user_ctx)
}

async fn store_latest_mail_event(user_ctx: &MailUserContext) {
    store_event_id(
        user_ctx.user_context(),
        MAIL_EVENT_TYPE_ID,
        CoreEventId::from(STORED_EVENT_ID),
    )
    .await
    .unwrap();
}

async fn social_category(tether: &mut Tether, marker: Option<EventId>) -> Label {
    let mut label = SystemLabel::CategorySocial
        .load(tether)
        .await
        .unwrap()
        .unwrap();
    label.last_unseen_message = marker;
    tether
        .write_tx::<_, _, StashError>(async |tx| label.save(tx).await)
        .await
        .unwrap();
    label
}

/// Green path: a category label carrying an unseen marker is cleared locally and the
/// server is told it was seen, using the latest stored mail event id.
#[tokio::test]
async fn marks_category_label_seen_and_notifies_server() {
    let (ctx, user_ctx) = setup().await;
    let mut tether = user_ctx.user_stash().connection();

    store_latest_mail_event(&user_ctx).await;
    let mut label = social_category(&mut tether, Some(EventId::from(UNSEEN_MARKER))).await;

    ctx.mock_post_label_seen(
        label.remote_id.clone().unwrap(),
        EventId::from(STORED_EVENT_ID),
        ResponseTemplate::new(200),
        1,
    )
    .await;

    LabelWithCounters::action_mark_seen(user_ctx.action_queue(), label.local_id.unwrap())
        .await
        .unwrap();
    user_ctx.execute_single_action().await.unwrap();

    label.reload(&tether).await.unwrap();
    assert_eq!(label.last_unseen_message, None);
}

/// A category label with nothing unseen is a no-op: no local change and, crucially,
/// no server call.
#[tokio::test]
async fn does_not_notify_server_when_nothing_unseen() {
    let (ctx, user_ctx) = setup().await;
    let mut tether = user_ctx.user_stash().connection();

    store_latest_mail_event(&user_ctx).await;
    let mut label = social_category(&mut tether, None).await;

    ctx.mock_post_label_seen(
        label.remote_id.clone().unwrap(),
        EventId::from(STORED_EVENT_ID),
        ResponseTemplate::new(200),
        0,
    )
    .await;

    LabelWithCounters::action_mark_seen(user_ctx.action_queue(), label.local_id.unwrap())
        .await
        .unwrap();
    user_ctx.execute_single_action().await.unwrap();

    label.reload(&tether).await.unwrap();
    assert_eq!(label.last_unseen_message, None);
}

/// Only category labels can be marked seen; any other label is rejected at enqueue
/// (`apply_local` runs synchronously) and the server is never contacted.
#[tokio::test]
async fn rejects_non_category_label() {
    let (ctx, user_ctx) = setup().await;
    let tether = user_ctx.user_stash().connection();

    store_latest_mail_event(&user_ctx).await;
    let inbox = SystemLabel::Inbox.load(&tether).await.unwrap().unwrap();

    ctx.mock_post_label_seen(
        inbox.remote_id.clone().unwrap(),
        EventId::from(STORED_EVENT_ID),
        ResponseTemplate::new(200),
        0,
    )
    .await;

    let Err(error) =
        LabelWithCounters::action_mark_seen(user_ctx.action_queue(), inbox.local_id.unwrap()).await
    else {
        panic!("marking a non-category label as seen must be rejected");
    };

    assert!(matches!(
        error,
        QueueActionError::Action(MailActionError::Label(LabelError::ExpectedCategoryLabel))
    ));

    let reloaded = Label::load(inbox.local_id.unwrap(), &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.last_unseen_message, inbox.last_unseen_message);
}

/// The local clear is optimistic: if the server rejects the request, the unseen
/// marker must be restored so the badge does not silently disappear.
#[tokio::test]
async fn restores_marker_when_server_rejects() {
    let (ctx, user_ctx) = setup().await;
    let mut tether = user_ctx.user_stash().connection();

    store_latest_mail_event(&user_ctx).await;
    let marker = EventId::from(UNSEEN_MARKER);
    let label = social_category(&mut tether, Some(marker.clone())).await;

    // A 4xx rejection fails immediately (5xx would be retried by the HTTP layer),
    // so the action errors once and the optimistic local clear must be rolled back.
    ctx.mock_post_label_seen(
        label.remote_id.clone().unwrap(),
        EventId::from(STORED_EVENT_ID),
        ResponseTemplate::new(422),
        1,
    )
    .await;

    LabelWithCounters::action_mark_seen(user_ctx.action_queue(), label.local_id.unwrap())
        .await
        .unwrap();
    let result = user_ctx.execute_single_action().await;
    assert!(result.is_err(), "server rejection must surface as an error");

    let reloaded = Label::load(label.local_id.unwrap(), &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.last_unseen_message, label.last_unseen_message);
}
