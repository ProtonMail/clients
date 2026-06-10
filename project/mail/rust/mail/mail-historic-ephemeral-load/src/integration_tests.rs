//! SQLite integration tests for ephemeral page persist and continuation resolution.

use std::sync::Arc;

use mail_action_queue::queue::{Queue, TokioTaskSpawner};
use mail_api::services::proton::common::MessageId;
use mail_search::{MailSearchService, MessageMetadata as SearchMessageMetadata};
use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashConfiguration, StashError};
use mail_task_service::TaskService;

use crate::checkpoint::EphemeralPageCheckpointWrite;
use crate::continuation::HistoricFetchContinuation;
use crate::ephemeral::persist_ephemeral_page;

async fn test_stash_with_mail_migrations() -> Stash<UserDb> {
    let mail_stash = Stash::new(StashConfiguration::test()).expect("test stash");
    mail_core_common::db::migrations::migrate_core_db(&mail_stash)
        .await
        .expect("core migrations");
    mail_common::db::offline_migrations::run(&mail_stash)
        .await
        .expect("mail migrations");
    mail_stash
}

async fn test_search_service() -> (Stash<UserDb>, MailSearchService) {
    let mail_stash = test_stash_with_mail_migrations().await;
    let task_service =
        Arc::new(TaskService::new(tokio::runtime::Handle::current()).expect("TaskService"));
    let svc = MailSearchService::new(mail_stash.clone(), task_service)
        .await
        .expect("MailSearchService::new");
    (mail_stash, svc)
}

async fn test_action_queue(mail_stash: &Stash<UserDb>) -> Queue<UserDb> {
    Queue::new(mail_stash.clone(), TokioTaskSpawner)
        .await
        .expect("action queue")
}

fn sample_search_metadata() -> SearchMessageMetadata {
    SearchMessageMetadata {
        subject: "historic load test".to_owned(),
        from: "sender@example.com".to_owned(),
        to: String::new(),
        cc: String::new(),
        bcc: String::new(),
    }
}

async fn blob_count(mail_stash: &Stash<UserDb>) -> i64 {
    let tether = mail_stash.connection();
    tether
        .sync_query(|conn| {
            conn.query_row("SELECT COUNT(*) FROM search_index_blobs", [], |row| {
                row.get(0)
            })
            .map_err(StashError::from)
        })
        .await
        .expect("blob count")
}

#[tokio::test]
async fn persist_ephemeral_page_writes_blobs_and_checkpoint_atomically() {
    let (mail_stash, svc) = test_search_service().await;

    let anchor_id = MessageId::from("ephemeral-integration-anchor");
    let prepared = svc
        .prepare_index_message_bodies_batch(&[(
            &anchor_id,
            "body for integration test",
            &sample_search_metadata(),
        )])
        .await
        .expect("prepare");
    assert!(!prepared.save_operations.is_empty());

    let action_queue = test_action_queue(&mail_stash).await;
    let saved = persist_ephemeral_page(
        &mail_stash,
        &action_queue,
        &svc,
        prepared,
        EphemeralPageCheckpointWrite::from_indexed_page(Some((1_700_000_100, anchor_id.clone()))),
        Vec::new(),
        None,
    )
    .await
    .expect("persist");
    assert_eq!(saved, 0, "no metadata rows in this test");

    assert!(blob_count(&mail_stash).await >= 1);

    let loaded = svc
        .load_ephemeral_historic_checkpoint()
        .await
        .expect("load")
        .expect("checkpoint");
    assert_eq!(loaded.anchor_time, 1_700_000_100);
    assert_eq!(loaded.anchor_message_id, anchor_id);
}

#[tokio::test]
async fn persist_ephemeral_page_unchanged_leaves_prior_checkpoint() {
    let (mail_stash, svc) = test_search_service().await;

    let prior_id = MessageId::from("prior-checkpoint-msg");
    svc.save_ephemeral_historic_checkpoint(1_600_000_000, &prior_id)
        .await
        .expect("seed checkpoint");

    let new_anchor = MessageId::from("new-indexed-msg");
    let prepared = svc
        .prepare_index_message_bodies_batch(&[(
            &new_anchor,
            "another body",
            &sample_search_metadata(),
        )])
        .await
        .expect("prepare");

    let action_queue = test_action_queue(&mail_stash).await;
    persist_ephemeral_page(
        &mail_stash,
        &action_queue,
        &svc,
        prepared,
        EphemeralPageCheckpointWrite::Unchanged,
        Vec::new(),
        None,
    )
    .await
    .expect("persist with unchanged checkpoint");

    let loaded = svc
        .load_ephemeral_historic_checkpoint()
        .await
        .expect("load")
        .expect("checkpoint");
    assert_eq!(loaded.anchor_time, 1_600_000_000);
    assert_eq!(loaded.anchor_message_id, prior_id);
}

#[tokio::test]
async fn persist_ephemeral_page_rolls_back_blobs_on_transaction_failure() {
    let (mail_stash, svc) = test_search_service().await;

    let anchor_id = MessageId::from("rollback-anchor");
    let prepared = svc
        .prepare_index_message_bodies_batch(&[(
            &anchor_id,
            "rollback body",
            &sample_search_metadata(),
        )])
        .await
        .expect("prepare");
    let save_operations = prepared.save_operations;
    assert!(!save_operations.is_empty());

    let mut tether = mail_stash.connection();
    let result: Result<(), StashError> = tether
        .quiet_write_tx(async |tx| {
            mail_search::save_blobs_in_write_tx(tx, save_operations).await?;
            Err(StashError::Custom(anyhow::anyhow!(
                "injected abort after blobs"
            )))
        })
        .await;
    assert!(result.is_err());
    assert_eq!(blob_count(&mail_stash).await, 0);
    assert!(
        svc.load_ephemeral_historic_checkpoint()
            .await
            .expect("load")
            .is_none()
    );
}

#[tokio::test]
async fn resolve_effective_continuation_prefers_explicit_over_db() {
    let (_mail_stash, svc) = test_search_service().await;

    svc.save_ephemeral_historic_checkpoint(1_111, &MessageId::from("db-stored-anchor"))
        .await
        .expect("seed");

    let explicit = HistoricFetchContinuation {
        anchor_time: 2_222,
        anchor_message_id: MessageId::from("explicit-anchor"),
    };
    let resolved = crate::resolve_effective_continuation(&svc, Some(explicit.clone()), true)
        .await
        .expect("resolve");
    let effective = resolved.expect("some continuation");
    assert_eq!(effective.anchor_time, explicit.anchor_time);
    assert_eq!(effective.anchor_message_id, explicit.anchor_message_id);
}

#[tokio::test]
async fn resolve_effective_continuation_loads_from_db_when_resuming() {
    let (_mail_stash, svc) = test_search_service().await;

    let stored_id = MessageId::from("resume-anchor-id");
    svc.save_ephemeral_historic_checkpoint(9_999_999, &stored_id)
        .await
        .expect("seed");

    let resolved = crate::resolve_effective_continuation(&svc, None, true)
        .await
        .expect("resolve")
        .expect("continuation");
    assert_eq!(resolved.anchor_time, 9_999_999);
    assert_eq!(resolved.anchor_message_id, stored_id);
}

#[tokio::test]
async fn persist_page_checkpoint_only_advances_checkpoint_without_blobs() {
    use crate::ephemeral::persist_page_checkpoint_only;

    let (mail_stash, svc) = test_search_service().await;
    let blobs_before = blob_count(&mail_stash).await;

    let anchor_id = MessageId::from("checkpoint-only-anchor");
    persist_page_checkpoint_only(
        &mail_stash,
        EphemeralPageCheckpointWrite::from_indexed_page(Some((1_800_000_000, anchor_id.clone()))),
        None,
    )
    .await
    .expect("checkpoint persist");

    assert_eq!(
        blob_count(&mail_stash).await,
        blobs_before,
        "checkpoint-only txn must not write index blobs"
    );

    let loaded = svc
        .load_ephemeral_historic_checkpoint()
        .await
        .expect("load")
        .expect("checkpoint");
    assert_eq!(loaded.anchor_time, 1_800_000_000);
    assert_eq!(loaded.anchor_message_id, anchor_id);
}
