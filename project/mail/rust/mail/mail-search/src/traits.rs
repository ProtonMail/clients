//! External traits that must be implemented by the consuming crate (mail-common)
//!
//! These traits define the boundary between mail-search and mail-common.
//! mail-search is self-contained with its own DB schema, but needs to
//! fetch message data (body, remote ID) from the main message store.

use async_trait::async_trait;
use mail_api::services::proton::common::MessageId;

use crate::error::SearchError;
use crate::intent::LocalMessageId;

/// Provides message data for search indexing
///
/// This trait is implemented by mail-common to provide access to message
/// bodies and remote IDs without mail-search needing to know about the
/// `Message` or `MessageBody` models.
#[async_trait]
pub trait MessageDataProvider: Send + Sync {
    /// Error type for data provider operations
    type Error: std::error::Error + Send + Sync + 'static;

    /// Get the decrypted message body for indexing
    ///
    /// Returns `None` if:
    /// - Message doesn't exist
    /// - Body hasn't been stored yet
    /// - Decryption failed
    ///
    /// Returns `Some((body, is_html))` where:
    /// - `body` is the decrypted message body content
    /// - `is_html` is `true` if the MIME type is `text/html`, `false` for `text/plain`
    async fn get_body(
        &self,
        message_id: LocalMessageId,
    ) -> Result<Option<(String, bool)>, Self::Error>;

    /// Get the remote (server) message ID
    ///
    /// Returns `None` if:
    /// - Message doesn't exist
    /// - Message hasn't been synced to server yet (e.g., local draft)
    async fn get_remote_id(
        &self,
        message_id: LocalMessageId,
    ) -> Result<Option<MessageId>, Self::Error>;

    /// Check if a message has local draft metadata (is being edited locally)
    ///
    /// Messages being edited locally are skipped during indexing because their
    /// content is incomplete. They will be indexed when sent.
    ///
    /// Note: This only skips drafts with local `DraftMetadata`. Drafts that
    /// exist but aren't being edited locally (e.g., synced from another device)
    /// will still be indexed.
    async fn has_local_draft_metadata(
        &self,
        message_id: LocalMessageId,
    ) -> Result<bool, Self::Error>;

    /// Get message metadata for indexing (subject, sender, recipients)
    ///
    /// Returns `None` if:
    /// - Message doesn't exist
    ///
    /// Returns `Some(metadata)` with subject and email addresses for search.
    async fn get_metadata(
        &self,
        message_id: LocalMessageId,
    ) -> Result<Option<MessageMetadata>, Self::Error>;

    /// Batch prepare multiple messages for indexing (optional optimization)
    ///
    /// Returns `Ok(Some(results))` when batch preparation is supported and succeeded.
    /// Returns `Ok(None)` when batch preparation is **not supported** — callers must fall back
    /// to individual preparation. Returns `Err` on failure.
    ///
    /// Implementations that support batch preparation must override this and return `Some`.
    /// The default returns `Ok(None)` to signal unsupported; callers that ignore this will
    /// receive `None` and must handle it explicitly.
    async fn batch_prepare_messages(
        &self,
        _message_ids: &[LocalMessageId],
    ) -> Result<Option<Vec<BatchPreparedMessage>>, Self::Error> {
        Ok(None)
    }

    /// Parse raw metadata strings into `MessageMetadata`
    fn parse_metadata_raw(
        &self,
        _subject: &str,
        _sender_json: &str,
        _to_list_json: Option<&str>,
        _cc_list_json: Option<&str>,
        _bcc_list_json: Option<&str>,
    ) -> Result<Option<MessageMetadata>, Self::Error> {
        Ok(None)
    }
}

/// MIME type values for `BatchPreparedMessage::body_mime_type`.
/// Contract between batch preparation (e.g. mail-common) and the worker.
pub const MIME_TYPE_PLAIN: i32 = 0;
pub const MIME_TYPE_HTML: i32 = 1;

/// Null byte separator between metadata fields in content hash. Prevents collisions
/// from concatenation (e.g. "a" + "b" vs "ab").
const METADATA_HASH_SEPARATOR: [u8; 1] = [0_u8];

/// Raw metadata strings from batch preparation (subject, `sender_json`, `to_list_json`, `cc_list_json`, `bcc_list_json`)
pub type RawMetadataStrings = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Result of batch preparing messages for indexing
#[derive(Debug, Clone)]
pub struct BatchPreparedMessage {
    pub message_id: LocalMessageId,
    pub remote_id: Option<MessageId>,
    // Raw body bytes - convert to String only after early checks pass (like old approach)
    pub body_raw: Option<Vec<u8>>,
    pub body_decryption_error: Option<String>,
    pub body_mime_type: Option<i32>, // MIME_TYPE_HTML = 1, MIME_TYPE_PLAIN = 0
    // Raw metadata strings - parse only when needed
    pub metadata_raw: Option<RawMetadataStrings>,
    pub has_local_draft: bool,
    pub stored_content_hash: Option<String>,
}

/// Message metadata for search indexing
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct MessageMetadata {
    /// Message subject
    pub subject: String,
    /// Sender email address
    pub from: String,
    /// To recipients (comma-separated email addresses)
    pub to: String,
    /// CC recipients (comma-separated email addresses)
    pub cc: String,
    /// BCC recipients (comma-separated email addresses)
    pub bcc: String,
}

impl MessageMetadata {
    /// Compute a content hash for duplicate detection
    ///
    /// This hash represents the searchable content of a message (body + metadata).
    /// If the hash matches a previously indexed message, we can skip re-indexing.
    ///
    /// Uses SHA256 directly on the raw bytes. Deterministic across toolchain versions
    /// (unlike `DefaultHasher`), which is required since hashes are persisted to `SQLite`.
    #[must_use]
    pub fn compute_content_hash(body: &str, metadata: Option<&Self>) -> String {
        use sha2::{Digest, Sha256};

        let mut sha256 = Sha256::new();
        sha256.update(body.as_bytes());

        if let Some(meta) = metadata {
            // Feed metadata fields directly into SHA256. Separators prevent collisions
            // from concatenation (e.g. "a" + "b" vs "ab").
            sha256.update(meta.subject.as_bytes());
            sha256.update(METADATA_HASH_SEPARATOR);
            sha256.update(meta.from.as_bytes());
            sha256.update(METADATA_HASH_SEPARATOR);
            sha256.update(meta.to.as_bytes());
            sha256.update(METADATA_HASH_SEPARATOR);
            sha256.update(meta.cc.as_bytes());
            sha256.update(METADATA_HASH_SEPARATOR);
            sha256.update(meta.bcc.as_bytes());
        }

        hex::encode(sha256.finalize())
    }
}

/// Trait for blob storage backends
///
/// This trait exists to decouple `mail-search` from the `mail_stash` crate,
/// allowing the search engine to remain storage-agnostic. While there's
/// currently only one implementation (`StashBlobStorage` in `mail-search/src/storage.rs`),
/// the trait enables unit testing with mock storage if needed.
///
/// Implementations must provide async load/save operations for index blobs.
#[async_trait]
pub trait BlobStorage: Send + Sync {
    /// Load a blob by name, returning None if not found
    async fn load(&self, name: &str) -> Result<Option<Vec<u8>>, SearchError>;

    /// Save a blob with the given name
    async fn save(&self, name: &str, data: &[u8]) -> Result<(), SearchError>;

    /// Delete a blob by name
    async fn delete(&self, name: &str) -> Result<bool, SearchError>;

    /// Clear all blobs from storage
    ///
    /// Removes all stored blobs, effectively clearing the entire index.
    /// Used by the `clear()` method to reset the search index.
    async fn clear_all(&self) -> Result<(), SearchError>;

    /// Save multiple blobs atomically in a single transaction
    ///
    /// This ensures that either all blobs are saved or none are, preventing
    /// orphaned blobs if the operation fails mid-way.
    ///
    /// Implementations that support transactions should use them here.
    /// Implementations that don't support transactions should fall back to
    /// saving blobs individually (which may not be fully atomic).
    async fn save_batch_atomic(&self, blobs: Vec<(String, Vec<u8>)>) -> Result<(), SearchError> {
        // Default implementation: save individually (non-transactional fallback)
        for (name, data) in blobs {
            self.save(&name, &data).await?;
        }
        Ok(())
    }
}
