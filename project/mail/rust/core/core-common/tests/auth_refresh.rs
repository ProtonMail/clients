use std::time::Duration;

use mail_core_api::services::proton::ProtonCore as _;
use mail_core_common::test_utils::test_context::TestContext;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_concurrent_refresh() {
    let ctx = TestContext::new().await;

    const PARALLEL: usize = 8;

    Mock::given(method("POST"))
        .and(path("/api/auth/v4/refresh"))
        .respond_with(move |_: &wiremock::Request| {
            ResponseTemplate::new(200)
                .set_body_json(json!({
                    "UID": "TEST_UID",
                    "AccessToken": "NEW_ACCESS",
                    "RefreshToken": "NEW_REFRESH",
                    "Scopes": ["full"],
                }))
                .set_delay(Duration::from_millis(200))
        })
        .with_priority(1)
        .named("/auth/v4/refresh — counted, slow")
        .expect(1)
        .mount(ctx.mock_server())
        .await;

    // catch the first request with the failed auth token,
    Mock::given(method("GET"))
        .and(path("/api/core/v4/events/latest"))
        .and(header("authorization", "Bearer ACCESSTOKEN"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(json!({
                    "Code": 401,
                    "Error": "Invalid access token",
                }))
                .set_delay(Duration::from_millis(200)),
        )
        .named("/events/latest — stale token returns 401")
        .expect(1..=PARALLEL as u64)
        .mount(ctx.mock_server())
        .await;

    // Requests carrying the freshly-refreshed bearer token succeed. Priority
    // 1 so the bearer match wins over the catch-all 401 above when both
    // would otherwise apply.
    Mock::given(method("GET"))
        .and(path("/api/core/v4/events/latest"))
        .and(header("authorization", "Bearer NEW_ACCESS"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({
                    "EventID": "abc",
                }))
                .set_delay(Duration::from_millis(200)),
        )
        .named("/events/latest — fresh token returns 200")
        .expect(PARALLEL as u64)
        .mount(ctx.mock_server())
        .await;

    let user_ctx = ctx.user_context().await;
    let session = user_ctx.session().clone();

    let (wait_tx, mut wait_rx) = tokio::sync::mpsc::channel(PARALLEL);
    let (start_tx, start_rx) = tokio::sync::mpsc::channel(1);

    // add one value to block all tasks on start_tx
    start_tx.send(()).await.unwrap();

    let handles: Vec<_> = (0..PARALLEL)
        .map(|_| {
            let session = session.clone();
            let start_tx = start_tx.clone();
            let wait_tx = wait_tx.clone();
            tokio::spawn(async move {
                wait_tx.send(()).await.unwrap();
                let _ = start_tx.send(()).await;
                session.get_events_latest().await
            })
        })
        .collect();

    // wait until all requests are ready to
    for _ in 0..PARALLEL {
        wait_rx.recv().await.unwrap();
    }

    // start all requests at the same time
    drop(start_rx);

    let mut errors = Vec::new();
    for h in handles {
        if let Err(e) = h.await.expect("task should not panic") {
            errors.push(format!("{e:?}"));
        }
    }

    assert!(
        errors.is_empty(),
        "all {PARALLEL} concurrent requests should succeed; {} failed: {errors:#?}",
        errors.len()
    );
}
