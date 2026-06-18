use mail_core_api::services::proton::{SessionId, UserId};
use mail_core_common::OnSessionDeletedResponse;
use mail_core_common::db::account::CoreSession;
use mail_core_common::models::ModelExtension;
use mail_core_common::services::SessionObserverService;
use mail_core_common::test_utils::test_context::TestContext;
use mail_stash::AccountDb;
use mail_stash::stash::{Stash, StashError, WriteTx};
use std::path::Path;
use std::time::Duration;

fn user_db_is_archived_or_removed(path: &Path) -> bool {
    !path.exists() || path.to_string_lossy().contains(".nuked")
}

async fn wait_for_user_db_archived_or_removed(path: &Path, timeout: Duration, message: &str) {
    let deadline = tokio::time::Instant::now() + timeout;
    while !user_db_is_archived_or_removed(path) {
        if tokio::time::Instant::now() >= deadline {
            panic!("{}", message);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_no_sessions_for_user(
    user_id: &UserId,
    account_stash: &Stash<AccountDb>,
    timeout: Duration,
    message: &str,
) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let tether = account_stash.connection();
        let count = CoreSession::find_by_user_id(user_id.clone(), &tether)
            .await
            .unwrap()
            .len();
        if count == 0 {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("{}", message);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
#[allow(unused_variables)]
async fn test_session_state() {
    let ctx = TestContext::new().await;
    let real_ctx = ctx.context();
    let user_ctx = ctx.user_context().await;
}

#[tokio::test]
#[allow(unused_variables)]
async fn test_session_state_watcher() {
    let ctx = TestContext::new().await;
    let real_ctx = ctx.context();
    let user_ctx = ctx.user_context().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_session_delete_subscriber() {
    let ctx = TestContext::new().await;
    let real_ctx = ctx.context();
    let user_ctx = ctx.user_context().await;

    let session_id = user_ctx.session_id().clone();
    let user_id = user_ctx.user_id().clone();
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<()>(1);
    let event_service = ctx.context().event_service();
    let session_observer_service = ctx.context.get_service::<SessionObserverService>();
    session_observer_service.on_session_deleted(
        event_service,
        move |deleted_session_id: SessionId, deleted_user_id: UserId| {
            let deleted_session_id = deleted_session_id.clone();
            let deleted_user_id = deleted_user_id.clone();
            let sender = sender.clone();
            let user_id = user_id.clone();
            let session_id = session_id.clone();
            async move {
                assert_eq!(deleted_user_id, user_id);
                assert_eq!(deleted_session_id, session_id);
                sender.send(()).await.unwrap();
                OnSessionDeletedResponse::Terminate
            }
        },
    );

    real_ctx
        .account_stash()
        .connection()
        .write_tx(async |tx: &WriteTx<'_, AccountDb>| {
            assert_eq!(CoreSession::all(tx).await.unwrap().len(), 1);
            assert!(
                CoreSession::delete_by_id(user_ctx.session_id().clone(), tx)
                    .await
                    .unwrap(),
            );
            Ok::<_, StashError>(())
        })
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn test_force_logout_account_locally_deletes_sessions() {
    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;
    let user_id = user_ctx.user_id().clone();

    let tether = ctx.context().account_stash().connection();
    assert_eq!(
        CoreSession::find_by_user_id(user_id.clone(), &tether)
            .await
            .unwrap()
            .len(),
        1
    );

    ctx.context()
        .force_logout_account_locally(user_id.clone())
        .await
        .unwrap();

    let tether = ctx.context().account_stash().connection();
    assert_eq!(
        CoreSession::find_by_user_id(user_id.clone(), &tether)
            .await
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_session_observer_triggers_full_logout_on_session_deletion() {
    // This test simulates a remote logout scenario (e.g., "log out from all devices")
    // where the session is deleted from the database, triggering the SessionObserverService
    // to perform a full logout and data cleanup.
    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;
    let user_id = user_ctx.user_id().clone();
    let session_id = user_ctx.session_id().clone();

    // Verify session exists before deletion
    let tether = ctx.context().account_stash().connection();
    assert_eq!(
        CoreSession::find_by_user_id(user_id.clone(), &tether)
            .await
            .unwrap()
            .len(),
        1
    );

    // Verify user database exists and has tables
    let user_db_path = ctx.context().user_db_path(&user_id);
    assert!(user_db_path.exists(), "User database should exist");

    // Delete the session from the database (simulating what happens when mail_muon's
    // AuthStore receives Auth::None after a failed token refresh on remote logout)
    ctx.context()
        .account_stash()
        .connection()
        .write_tx(async |tx: &WriteTx<'_, AccountDb>| {
            CoreSession::delete_by_id(session_id.clone(), tx).await?;
            Ok::<_, StashError>(())
        })
        .await
        .unwrap();

    // Verify that SessionObserverService automatically performed full cleanup:
    wait_for_no_sessions_for_user(
        &user_id,
        ctx.context().account_stash(),
        Duration::from_secs(5),
        "Session should be deleted",
    )
    .await;

    // User database should be archived/removed (logout_and_delete_user_data does this)
    // The database file gets renamed with a timestamp and .nuked extension
    wait_for_user_db_archived_or_removed(
        &user_db_path,
        Duration::from_secs(5),
        "User database should be archived or removed",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_manual_logout_with_session_observer_double_cleanup() {
    // This test verifies that manually calling logout_account() (which deletes sessions)
    // doesn't cause issues when SessionObserverService also triggers logout_and_delete_user_data().
    // This ensures the double-cleanup is idempotent and doesn't cause crashes or errors.

    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;
    let user_id = user_ctx.user_id().clone();

    // Verify initial state - session exists
    let tether = ctx.context().account_stash().connection();
    assert_eq!(
        CoreSession::find_by_user_id(user_id.clone(), &tether)
            .await
            .unwrap()
            .len(),
        1,
        "Session should exist before manual logout"
    );

    // Verify user database exists
    let user_db_path = ctx.context().user_db_path(&user_id);
    assert!(user_db_path.exists(), "User database should exist");

    // Manually call logout_account() as a user would do through the app
    // This will:
    // 1. Call API logout (mocked to succeed)
    // 2. Delete sessions from DB via force_logout_account_locally()
    // 3. Trigger SessionObserver which calls logout_and_delete_user_data()
    let logout_result = ctx.context().logout_account(user_id.clone()).await;

    // The logout should succeed without errors
    assert!(
        logout_result.is_ok(),
        "Manual logout should succeed: {:?}",
        logout_result.err()
    );

    // Verify final state: sessions are deleted and user data is cleaned up
    wait_for_no_sessions_for_user(
        &user_id,
        ctx.context().account_stash(),
        Duration::from_secs(5),
        "All sessions should be deleted after manual logout",
    )
    .await;

    // The SessionObserver's logout_and_delete_user_data() should have archived/removed it
    wait_for_user_db_archived_or_removed(
        &user_db_path,
        Duration::from_secs(5),
        "User database should be archived or removed by SessionObserver",
    )
    .await;

    // Why this double-cleanup is safe and idempotent:
    // 1. Session deletion: When logout_account() is called the second time by SessionObserver,
    //    it finds no sessions (already deleted) and returns without error
    // 2. User database cleanup: logout_and_delete_user_data() gets user_context from cache first
    //    (before sessions are removed from cache), ensuring tables are always dropped.
    //    Operations like drop_database_tables() and file removal are wrapped in error handling.
    // 3. Active context removal: Removing from active_user_contexts multiple times is safe -
    //    the second removal is a no-op
    // 4. Task cancellation: cancel_user_tasks() is idempotent - cancelling already-cancelled tasks is harmless
}
