//! Stash-based blob storage for Foundation Search
//!
//! This module provides a `BlobStorage` implementation that uses Stash
//! (`SQLite`) for persisting search index blobs.

use std::io::{Read, Write};

use async_trait::async_trait;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use mail_stash::UserDb;
use mail_stash::stash::Stash;
use tracing::debug;

use crate::error::SearchError;
use crate::traits::BlobStorage;

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

fn compress_gzip(data: &[u8]) -> Result<Vec<u8>, SearchError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| SearchError::BlobStorage(format!("Compress failed: {e}")))?;
    encoder
        .finish()
        .map_err(|e| SearchError::BlobStorage(format!("Compress finish failed: {e}")))
}

fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, SearchError> {
    let mut decoder = GzDecoder::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| SearchError::BlobStorage(format!("Decompress failed: {e}")))?;
    Ok(out)
}

fn is_gzip(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == GZIP_MAGIC[0] && data[1] == GZIP_MAGIC[1]
}

/// Stash-based implementation of `BlobStorage`
///
/// Stores search index blobs in `SQLite` via the Stash connection pool.
#[derive(Clone)]
pub struct StashBlobStorage {
    mail_stash: Stash<UserDb>,
}

impl StashBlobStorage {
    /// Create a new Stash-based blob storage
    #[must_use]
    pub fn new(mail_stash: Stash<UserDb>) -> Self {
        Self { mail_stash }
    }

    /// Get a reference to the underlying Stash instance
    #[must_use]
    pub fn mail_stash(&self) -> &Stash<UserDb> {
        &self.mail_stash
    }

    /// Save multiple blobs atomically in a single transaction
    ///
    /// This ensures that either all blobs are saved or none are, preventing
    /// orphaned blobs if the operation fails mid-way.
    pub async fn save_batch_atomic(
        &self,
        blobs: Vec<(String, Vec<u8>)>,
    ) -> Result<(), SearchError> {
        use mail_stash::params;
        use mail_stash::stash::StashError as SE;

        if blobs.is_empty() {
            return Ok(());
        }

        let mut tether = self
            .mail_stash
            .connection()
            .await
            .map_err(|e| SearchError::BlobStorage(format!("Failed to get connection: {e}")))?;

        let timestamp = chrono::Utc::now().timestamp();
        let count = blobs.len();
        tether
            .write_tx::<_, (), SE>(async |bond| {
                for (name, data) in blobs {
                    let stored = compress_gzip(&data)
                        .map_err(|e| mail_stash::stash::StashError::Custom(anyhow::anyhow!("{e}")))?;
                    bond.execute(
                        "INSERT OR REPLACE INTO search_index_blobs (blob_name, blob_data, updated_at)
                         VALUES (?1, ?2, ?3)",
                        params![name, stored, timestamp],
                    )
                    .await?;
                }
                Ok(())
            })
            .await
            .map_err(|e| SearchError::BlobStorage(format!("Transaction failed: {e}")))?;

        debug!("Saved {} blobs atomically", count);
        Ok(())
    }
}

#[async_trait]
impl BlobStorage for StashBlobStorage {
    async fn load(&self, name: &str) -> Result<Option<Vec<u8>>, SearchError> {
        let name_owned = name.to_owned();
        let tether = self
            .mail_stash
            .connection()
            .await
            .map_err(|e| SearchError::BlobStorage(format!("Failed to get connection: {e}")))?;

        let raw = tether
            .sync_query(move |conn| {
                use mail_stash::rusqlite::OptionalExtension;
                conn.query_row(
                    "SELECT blob_data FROM search_index_blobs WHERE blob_name = ?1",
                    [&name_owned],
                    |row| row.get::<_, Vec<u8>>(0),
                )
                .optional()
                .map_err(|e| {
                    mail_stash::stash::StashError::Custom(anyhow::anyhow!(
                        "Failed to load blob '{}': {}",
                        name_owned,
                        e
                    ))
                })
            })
            .await
            .map_err(|e| SearchError::BlobStorage(format!("Query failed: {e}")))?;

        let blob = match raw {
            Some(data) if is_gzip(&data) => Some(decompress_gzip(&data)?),
            other => other,
        };

        debug!(
            "Loaded blob '{}' ({} bytes)",
            name,
            blob.as_ref().map_or(0, Vec::len)
        );
        Ok(blob)
    }

    async fn save(&self, name: &str, data: &[u8]) -> Result<(), SearchError> {
        use mail_stash::params;
        use mail_stash::stash::StashError as SE;

        let data_len = data.len();
        let name_owned = name.to_owned();
        let data_owned = compress_gzip(data)?;
        let compressed_len = data_owned.len();
        let timestamp = chrono::Utc::now().timestamp();

        let mut tether = self
            .mail_stash
            .connection()
            .await
            .map_err(|e| SearchError::BlobStorage(format!("Failed to get connection: {e}")))?;

        tether
            .write_tx::<_, (), SE>(async |bond| {
                bond.execute(
                    "INSERT OR REPLACE INTO search_index_blobs (blob_name, blob_data, updated_at)
                     VALUES (?1, ?2, ?3)",
                    params![name_owned, data_owned, timestamp],
                )
                .await?;
                Ok(())
            })
            .await
            .map_err(|e| SearchError::BlobStorage(format!("Transaction failed: {e}")))?;

        debug!(
            "Saved blob '{}' ({} bytes -> {} compressed)",
            name, data_len, compressed_len
        );
        Ok(())
    }

    async fn delete(&self, name: &str) -> Result<bool, SearchError> {
        use mail_stash::params;
        use mail_stash::stash::StashError as SE;

        let name_owned = name.to_owned();

        let mut tether = self
            .mail_stash
            .connection()
            .await
            .map_err(|e| SearchError::BlobStorage(format!("Failed to get connection: {e}")))?;

        let deleted = tether
            .write_tx::<_, bool, SE>(async |bond| {
                let rows = bond
                    .execute(
                        "DELETE FROM search_index_blobs WHERE blob_name = ?1",
                        params![name_owned],
                    )
                    .await?;
                Ok(rows > 0)
            })
            .await
            .map_err(|e| SearchError::BlobStorage(format!("Transaction failed: {e}")))?;

        if deleted {
            debug!("Deleted blob '{}'", name);
        }

        Ok(deleted)
    }

    async fn clear_all(&self) -> Result<(), SearchError> {
        use mail_stash::stash::StashError as SE;

        let mut tether = self
            .mail_stash
            .connection()
            .await
            .map_err(|e| SearchError::BlobStorage(format!("Failed to get connection: {e}")))?;

        let deleted_count = tether
            .write_tx::<_, usize, SE>(async |bond| {
                let count = bond
                    .execute("DELETE FROM search_index_blobs", vec![])
                    .await?;
                Ok(count)
            })
            .await
            .map_err(|e| SearchError::BlobStorage(format!("Transaction failed: {e}")))?;

        debug!("Cleared all {} blobs from storage", deleted_count);
        Ok(())
    }

    async fn save_batch_atomic(&self, blobs: Vec<(String, Vec<u8>)>) -> Result<(), SearchError> {
        // Use the existing implementation that provides transactional guarantees
        StashBlobStorage::save_batch_atomic(self, blobs).await
    }
}
