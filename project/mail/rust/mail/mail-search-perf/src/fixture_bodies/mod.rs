//! Fixture-based message body provider for search performance testing.
//!.
//! This module provides email bodies
//! from one of three sources:
//! 1. A local JSONL fixture file
//! 2. A remote batch API with progressive loading
//! 3. Chunked HTTP files with real bodies keyed by `remote_id` (preferred)
//!

mod chunked;
mod crypto;
mod sequential;
mod util;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde::Deserialize;

/// Errors that can occur when loading fixtures
#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Fixture file is empty")]
    EmptyFixture,
    #[error("Fixtures not initialized - call initialize() or initialize_from_api() first")]
    NotInitialized,
    #[error("Fixtures already initialized")]
    AlreadyInitialized,
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    #[error("Remote fixture config JSON: {0}")]
    ConfigJson(String),
}

/// Configuration for the integer-ID batch HTTP fixture source (`initialize_from_api`).
#[derive(Clone, Debug, Deserialize)]
pub struct BatchApiFixtureConfig {
    /// POST endpoint that accepts JSON `{"ids":[...]}` and returns encrypted bodies.
    pub api_url: String,
    /// 64 hex chars (32 bytes) AES-256 key matching your fixture ciphertext.
    pub encryption_key_hex: String,
}

/// Configuration for the chunked HTTP fixture source (`initialize_real_bodies_api`).
#[derive(Clone, Debug, Deserialize)]
pub struct ChunkedBodiesFixtureConfig {
    /// Base URL with no trailing slash; must expose `_index.json` and chunk paths under it.
    pub base_url: String,
    /// Same key material as used when encrypting bodies in the chunks.
    pub encryption_key_hex: String,
}

/// Remote fixture URLs and keys from a single JSON file (`batch_api` / `chunked_bodies` blocks).
///
/// See the module-level documentation above for a JSON example.
#[derive(Clone, Debug, Deserialize)]
pub struct RemoteFixtureConfigFile {
    /// Config for [`initialize_from_api`] when present.
    #[serde(default)]
    pub batch_api: Option<BatchApiFixtureConfig>,
    /// Config for [`initialize_real_bodies_api`] when present.
    #[serde(default)]
    pub chunked_bodies: Option<ChunkedBodiesFixtureConfig>,
}

/// Read [`RemoteFixtureConfigFile`] from a path (UTF-8 JSON).
pub fn load_remote_fixture_config_from_path(
    path: &Path,
) -> Result<RemoteFixtureConfigFile, FixtureError> {
    let file = File::open(path).map_err(|e| FixtureError::IoError(e.to_string()))?;
    serde_json::from_reader(BufReader::new(file))
        .map_err(|e| FixtureError::ConfigJson(e.to_string()))
}

/// Substitute body for perf fetch when fixtures or chunked real bodies are active.
///
pub fn try_substitute_perf_body(
    remote_id: &str,
) -> Result<Option<crate::SubstituteBody>, FixtureError> {
    if chunked::is_real_bodies_initialized() {
        let body = chunked::get_body_for_remote_id(remote_id)?;
        return Ok(Some(crate::SubstituteBody {
            body,
            mime: crate::DeclaredFixtureMime::TextHtml,
        }));
    }
    if sequential::is_initialized() {
        return sequential::get_next_substitute_body().map(Some);
    }
    Ok(None)
}

pub use chunked::{
    RealBodiesStats, RealBodiesStore, get_body_for_remote_id, initialize_real_bodies_api,
    is_real_bodies_initialized, is_real_bodies_loading_complete, real_bodies_loaded,
};
pub use sequential::{
    FixtureEmail, FixtureSource, FixtureStats, FixtureStore, current_index, expected_total,
    get_next_body, get_next_substitute_body, initialize, initialize_from_api, is_initialized,
    is_loading_complete, reset_index, total_fixtures, wait_for_bodies,
};
