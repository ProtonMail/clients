//! Message data provider for search indexing
//!
//! Implements the `MessageDataProvider` trait from mail-search,
//! providing access to message body and remote ID data.

use async_trait::async_trait;
use mail_api::services::proton::common::MessageId;
use mail_core_common::models::{ModelExtension, ModelIdExtension};
use mail_crypto_inbox::mail_crypto_inbox_mime::ProcessedBodyType;
use mail_crypto_inbox::message::DecryptedBody;
use mail_search::MessageDataProvider;
use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashError};

use crate::datatypes::LocalMessageId as MailLocalMessageId;
use crate::models::{DraftMetadata, Message, MessageBodyMetadata, MessageMimeType, RawMessageBody};
use mail_search::MessageMetadata;

/// Stash-based message data provider
///
/// Provides message data (body, remote ID) for search indexing
/// by querying the Message and MessageBody models.
#[derive(Clone)]
pub struct StashMessageDataProvider {
    mail_stash: Stash<UserDb>,
}

impl StashMessageDataProvider {
    /// Create a new message data provider
    pub fn new(mail_stash: Stash<UserDb>) -> Self {
        Self { mail_stash }
    }
}

#[async_trait]
impl MessageDataProvider for StashMessageDataProvider {
    type Error = StashError;

    async fn get_body(
        &self,
        message_id: mail_search::LocalMessageId,
    ) -> Result<Option<(String, bool)>, Self::Error> {
        let tether = self.mail_stash.connection();

        // Convert u64 to mail-common's LocalMessageId
        let local_id: MailLocalMessageId = message_id.into();

        // Load the raw message body and process it (similar to DecryptedMessageBody::from_raw_message_body)
        let Some(raw_body) = RawMessageBody::load(local_id, &tether).await? else {
            return Ok(None);
        };

        // Process the body to extract text content (for MIME messages, this excludes attachments)
        let (processed_body, is_html) = match raw_body.into_raw_decrypted_body() {
            Ok(raw_decrypted_body) => {
                match raw_decrypted_body.processed_body() {
                    Ok(decrypted_body) => match decrypted_body {
                        DecryptedBody::Plain(text) => {
                            // For non-MIME messages, MessageBodyMetadata.mime_type is the actual
                            // content type when present. When missing, treat as plain.
                            let metadata =
                                MessageBodyMetadata::for_message(local_id, &tether).await?;
                            let is_html = metadata
                                .map(|m| {
                                    MessageMimeType::from_api(m.mime_type, || {
                                        MessageMimeType::TextPlain
                                    })
                                })
                                .map(|mime_type| matches!(mime_type, MessageMimeType::TextHtml))
                                .unwrap_or(false);
                            (text, is_html)
                        }
                        DecryptedBody::Mime(mime) => {
                            // For MIME-encrypted messages, MessageBodyMetadata.mime_type
                            // is "multipart/mixed" — not the actual body content type.
                            // Use the decoded body's own type instead.
                            let is_html = matches!(mime.mime_body_type, ProcessedBodyType::Html);
                            (mime.body, is_html)
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            "Failed to process body for message {}: {}, skipping index",
                            local_id,
                            e
                        );
                        return Ok(None);
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Decryption failed for message {}: {}", local_id, e.error);
                return Ok(None);
            }
        };

        Ok(Some((processed_body, is_html)))
    }

    async fn get_remote_id(
        &self,
        message_id: mail_search::LocalMessageId,
    ) -> Result<Option<MessageId>, Self::Error> {
        let tether = self.mail_stash.connection();

        // Convert u64 to mail-common's LocalMessageId
        let local_id: MailLocalMessageId = message_id.into();

        Message::local_id_counterpart(local_id, &tether).await
    }

    async fn has_local_draft_metadata(
        &self,
        message_id: mail_search::LocalMessageId,
    ) -> Result<bool, Self::Error> {
        let tether = self.mail_stash.connection();

        // Convert u64 to mail-common's LocalMessageId
        let local_id: MailLocalMessageId = message_id.into();

        // Check if there's a DraftMetadata record for this message
        // This indicates the draft is being edited locally
        let draft_metadata = DraftMetadata::find_by_message_id(local_id, &tether).await?;

        Ok(draft_metadata.is_some())
    }

    async fn get_metadata(
        &self,
        message_id: mail_search::LocalMessageId,
    ) -> Result<Option<MessageMetadata>, Self::Error> {
        let tether = self.mail_stash.connection();

        // Convert u64 to mail-common's LocalMessageId
        let local_id: MailLocalMessageId = message_id.into();

        let message = Message::find_by_id(local_id, &tether).await?;

        let Some(message) = message else {
            return Ok(None);
        };

        // Extract email addresses from sender and recipients
        let from = message.sender.address.as_clear_text_str().to_string();
        let to = message
            .to_list
            .iter()
            .map(|r| r.address.as_clear_text_str())
            .collect::<Vec<_>>()
            .join(", ");
        let cc = message
            .cc_list
            .iter()
            .map(|r| r.address.as_clear_text_str())
            .collect::<Vec<_>>()
            .join(", ");
        let bcc = message
            .bcc_list
            .iter()
            .map(|r| r.address.as_clear_text_str())
            .collect::<Vec<_>>()
            .join(", ");

        Ok(Some(MessageMetadata {
            subject: message.subject,
            from,
            to,
            cc,
            bcc,
        }))
    }
}
