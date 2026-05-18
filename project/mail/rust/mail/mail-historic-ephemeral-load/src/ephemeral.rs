use std::sync::Arc;
use std::time::Instant;

use crate::continuation::{HistoricFetchContinuation, resolve_effective_continuation};
use futures::stream::{self, StreamExt};
use mail_api::MAX_PAGE_ELEMENT_COUNT;
use mail_api::services::proton::ProtonMail;
use mail_api::services::proton::common::MessageId;
use mail_api::services::proton::requests::GetMessagesOptions;
use mail_common::datatypes::labels::{ScrollOrderDir, ScrollOrderField};
use mail_common::datatypes::{EncryptedMessageBody, SystemLabelId};
use mail_common::models::MessageBodyMetadata;
use mail_common::{MailContextError, MailUserContext};
use mail_core_api::services::proton::LabelId;
use mail_crypto_inbox::message::{DecryptableMessage as _, DecryptedBody};
use mail_crypto_inbox::proton_crypto;
use mail_crypto_inbox::proton_crypto_account::keys::AddressKeySelector;
use mail_html_transformer::html_to_text_fast;
use mail_search::MessageMetadata;
use tracing::info;

use crate::ephemeral_timing::{EphemeralTimingCollector, EphemeralTimingStats};

#[derive(Debug, Clone)]
pub struct EphemeralHistoricLoadResult {
    pub messages_fetched: usize,
    pub messages_indexed: usize,
    pub messages_skipped_missing_body: usize,
    pub oldest_message_time: Option<u64>,
    pub oldest_message_remote_id: Option<String>,
    pub timing: EphemeralTimingStats,
}

/// Persists or clears the SQLite checkpoint from the oldest **indexed** message in the batch.
///
/// If no message was indexed (e.g. all body fetches failed), the checkpoint is cleared so a
/// later `resume_from_checkpoint` does not reuse a stale metadata cursor.
async fn persist_checkpoint_after_run(
    search_service: &mail_search::MailSearchService,
    oldest_saved: Option<(u64, MessageId)>,
) -> Result<(), MailContextError> {
    match oldest_saved {
        Some((anchor_time, anchor_message_id)) => {
            search_service
                .save_ephemeral_historic_checkpoint(anchor_time, &anchor_message_id)
                .await
                .map_err(|e| {
                    MailContextError::Other(anyhow::anyhow!(
                        "Ephemeral historic checkpoint: failed to save: {e}"
                    ))
                })?;
        }
        None => {
            search_service
                .clear_ephemeral_historic_checkpoint()
                .await
                .map_err(|e| {
                    MailContextError::Other(anyhow::anyhow!(
                        "Ephemeral historic checkpoint: failed to clear: {e}"
                    ))
                })?;
        }
    }
    Ok(())
}

/// Ephemeral historic load: All Mail metadata + bodies → Foundation Search (no message SQLite writes).
///
/// When `resume_from_checkpoint` is true and `continuation` is `None`, loads the saved anchor from
/// SQLite; if no row exists, starts from the newest messages. After each run, persists the oldest
/// **indexed** message as the next anchor (or clears the row if nothing was indexed).
pub async fn ephemeral_index_only_messages(
    user_ctx: &Arc<MailUserContext>,
    max_messages: usize,
    page_size: usize,
    concurrent_body_fetches: usize,
    continuation: Option<HistoricFetchContinuation>,
    resume_from_checkpoint: bool,
) -> Result<EphemeralHistoricLoadResult, MailContextError> {
    let mut timing = EphemeralTimingCollector::default();
    let remote_label_id = LabelId::all_mail();
    let search_service = user_ctx.search_service();

    let effective_continuation =
        resolve_effective_continuation(search_service, continuation, resume_from_checkpoint)
            .await?;

    let session = user_ctx.session();
    let effective_page_size = page_size.min(MAX_PAGE_ELEMENT_COUNT);

    info!(
        "Ephemeral historic load: fetch + decrypt bodies from API (no SQLite message writes, concurrency={})",
        concurrent_body_fetches
    );

    let mut total_pages: usize = if effective_continuation.is_some() {
        1
    } else {
        0
    };
    let mut last_message_id = effective_continuation
        .as_ref()
        .map(|c| c.anchor_message_id.clone());
    let mut last_message_time = effective_continuation.as_ref().map(|c| c.anchor_time);
    let mut page_requests: u32 = 0;
    let mut total_fetched = 0usize;
    let mut total_indexed = 0usize;
    let mut total_skipped_missing_body = 0usize;
    let mut oldest_saved: Option<(u64, MessageId)> = None;

    if let Some(c) = &effective_continuation {
        info!(
            "Resuming ephemeral historic fetch from anchor time={} id={}",
            c.anchor_time, c.anchor_message_id
        );
    }

    loop {
        if total_fetched >= max_messages {
            break;
        }

        let page_start = Instant::now();
        page_requests = page_requests.saturating_add(1);

        let mut opts = GetMessagesOptions {
            label_id: Some(vec![remote_label_id.clone()]),
            page_size: if total_pages == 0 {
                effective_page_size as u64
            } else {
                (effective_page_size as u64) + 1
            },
            unread: mail_common::datatypes::ReadFilter::All.into(),
            desc: ScrollOrderDir::Desc.as_api_desc(),
            sort: ScrollOrderField::Time.as_api_sort(),
            ..Default::default()
        };

        if total_pages > 0 {
            let anchor_time = last_message_time.expect("anchor_time should exist after first page");
            let anchor_id = last_message_id
                .as_ref()
                .expect("anchor_id should exist after first page")
                .clone();
            opts.anchor = Some(anchor_time);
            opts.anchor_id = Some(anchor_id);
        }

        let metadata_start = Instant::now();
        let response = ProtonMail::get_messages(session, opts).await?;
        let metadata_elapsed = metadata_start.elapsed();

        if response.messages.is_empty() {
            break;
        }

        let mut messages = response.messages;
        if total_pages > 0
            && !messages.is_empty()
            && let Some(last_id) = &last_message_id
        {
            if messages[0].id == *last_id {
                messages.remove(0);
            } else if messages.len() > effective_page_size {
                messages.pop();
            }
        }

        if messages.is_empty() {
            break;
        }

        let remaining = max_messages.saturating_sub(total_fetched);
        if messages.len() > remaining {
            messages.truncate(remaining);
        }

        let page_fetched = messages.len();
        total_fetched += page_fetched;

        let body_start = Instant::now();
        let mut page_docs: Vec<(MessageId, String, MessageMetadata)> =
            Vec::with_capacity(page_fetched);

        let message_ids: Vec<_> = messages.iter().map(|m| m.id.clone()).collect();

        let session_clone = session.clone();
        let fetched_bodies: std::collections::HashMap<String, Result<_, _>> =
            stream::iter(message_ids)
                .map(|mid| {
                    let s = session_clone.clone();
                    let key = mid.to_string();
                    async move {
                        (
                            key,
                            ProtonMail::get_message(&s, mid).await.map(|r| r.message),
                        )
                    }
                })
                .buffer_unordered(concurrent_body_fetches)
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .collect();

        let pgp = proton_crypto::new_pgp_provider();

        for meta_msg in &messages {
            let Some(body_result) = fetched_bodies.get(meta_msg.id.as_str()) else {
                tracing::warn!("Body fetch result missing for {}", meta_msg.id);
                total_skipped_missing_body += 1;
                continue;
            };
            let api_msg = match body_result {
                Ok(m) => m.clone(),
                Err(e) => {
                    tracing::warn!("Failed to fetch body for {}: {e}", meta_msg.id);
                    total_skipped_missing_body += 1;
                    continue;
                }
            };

            let remote_id = meta_msg.id.clone();
            let address_id = api_msg.metadata.address_id.clone();

            let encrypted = EncryptedMessageBody {
                encrypted_body: api_msg.body.body,
                metadata: MessageBodyMetadata {
                    remote_message_id: Some(remote_id.clone()),
                    mime_type: api_msg.body.mime_type.into(),
                    ..Default::default()
                },
            };

            let tether = user_ctx.user_stash().connection();
            let address_keys = user_ctx
                .crypto_key_service()
                .load_with_tether(user_ctx.user_context(), &tether)
                .address_keys(&pgp, &address_id)
                .await
                .map(AddressKeySelector::into_raw_keys)
                .map_err(|e| {
                    MailContextError::Other(anyhow::anyhow!(
                        "Key loading failed for {}: {e}",
                        remote_id
                    ))
                })?;
            drop(tether);

            let decrypt_start = Instant::now();
            let raw_decrypted = match encrypted.decrypt(&pgp, &address_keys) {
                Ok(raw) => raw,
                Err(e) => {
                    timing.record_decrypt(decrypt_start.elapsed());
                    tracing::warn!("Decrypt failed for {}: {e}", remote_id);
                    total_skipped_missing_body += 1;
                    continue;
                }
            };

            let decrypted_body = match raw_decrypted.processed_body() {
                Ok(body) => body,
                Err(e) => {
                    timing.record_decrypt(decrypt_start.elapsed());
                    tracing::warn!("Body processing failed for {}: {e}", remote_id);
                    total_skipped_missing_body += 1;
                    continue;
                }
            };
            timing.record_decrypt(decrypt_start.elapsed());

            let body_text = match &decrypted_body {
                DecryptedBody::Plain(text) => {
                    let strip_start = Instant::now();
                    let stripped = html_to_text_fast(text);
                    timing.record_html_strip(strip_start.elapsed());
                    stripped
                }
                DecryptedBody::Mime(_) => {
                    let strip_start = Instant::now();
                    let stripped = html_to_text_fast(decrypted_body.body());
                    timing.record_html_strip(strip_start.elapsed());
                    stripped
                }
            };

            push_doc(
                &mut page_docs,
                &mut oldest_saved,
                meta_msg,
                remote_id,
                body_text,
            );
        }
        let body_elapsed = body_start.elapsed();

        let index_start = Instant::now();
        if !page_docs.is_empty() {
            let refs = page_docs
                .iter()
                .map(|(rid, body, meta)| (rid, body.as_str(), meta))
                .collect::<Vec<_>>();
            user_ctx
                .search_service()
                .index_message_bodies_batch(&refs)
                .await
                .map_err(|e| {
                    MailContextError::Other(anyhow::anyhow!("Direct batch index failed: {e}"))
                })?;
            timing.record_index_only(index_start.elapsed(), refs.len());
            total_indexed += refs.len();
        }
        let index_elapsed = index_start.elapsed();

        if let Some(anchor) = messages.last() {
            last_message_id = Some(anchor.id.clone());
            last_message_time = Some(anchor.time);
        }
        total_pages = total_pages.saturating_add(1);

        info!(
            "Ephemeral page {}: fetched={} indexed={} skipped={} | metadata={:.1}ms bodies={:.1}ms index={:.1}ms total={:.1}ms",
            page_requests,
            page_fetched,
            page_docs.len(),
            page_fetched - page_docs.len(),
            metadata_elapsed.as_secs_f64() * 1000.0,
            body_elapsed.as_secs_f64() * 1000.0,
            index_elapsed.as_secs_f64() * 1000.0,
            page_start.elapsed().as_secs_f64() * 1000.0,
        );
        info!(
            "Ephemeral cumulative: fetched={} indexed={} skipped_no_body={} pages={}",
            total_fetched, total_indexed, total_skipped_missing_body, page_requests
        );

        if page_fetched < effective_page_size {
            break;
        }
    }

    let (oldest_message_time, oldest_message_remote_id) = match &oldest_saved {
        Some((t, id)) => (Some(*t), Some(id.to_string())),
        None => (None, None),
    };

    persist_checkpoint_after_run(search_service, oldest_saved).await?;

    Ok(EphemeralHistoricLoadResult {
        messages_fetched: total_fetched,
        messages_indexed: total_indexed,
        messages_skipped_missing_body: total_skipped_missing_body,
        oldest_message_time,
        oldest_message_remote_id,
        timing: timing.snapshot(),
    })
}

fn push_doc(
    page_docs: &mut Vec<(MessageId, String, MessageMetadata)>,
    oldest_saved: &mut Option<(u64, MessageId)>,
    message: &mail_api::services::proton::prelude::MessageMetadata,
    remote_id: MessageId,
    body: String,
) {
    let metadata = MessageMetadata {
        subject: message.subject.clone(),
        from: message.sender.address.as_clear_text_str().to_owned(),
        to: message
            .to_list
            .iter()
            .map(|r| r.address.as_clear_text_str())
            .collect::<Vec<_>>()
            .join(","),
        cc: message
            .cc_list
            .iter()
            .map(|r| r.address.as_clear_text_str())
            .collect::<Vec<_>>()
            .join(","),
        bcc: message
            .bcc_list
            .iter()
            .map(|r| r.address.as_clear_text_str())
            .collect::<Vec<_>>()
            .join(","),
    };

    let t = message.time;
    let replace = match oldest_saved {
        None => true,
        Some((ot, _)) if t < *ot => true,
        Some((ot, oid)) if t == *ot && remote_id < *oid => true,
        _ => false,
    };
    if replace {
        *oldest_saved = Some((t, remote_id.clone()));
    }

    page_docs.push((remote_id, body, metadata));
}
