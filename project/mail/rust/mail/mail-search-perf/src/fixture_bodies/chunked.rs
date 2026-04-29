//! Chunked HTTPS real bodies (`initialize_real_bodies_api`, `get_body_for_remote_id`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;

use serde::Deserialize;
use tracing::{debug, error, info};

use super::ChunkedBodiesFixtureConfig;
use super::FixtureError;
use super::crypto::{decompress_gzip, decrypt_body};
use super::util::{acquire_mutex, acquire_read, acquire_write};

static REAL_BODIES_STORE: RwLock<Option<Arc<RealBodiesStore>>> = RwLock::new(None);
/// Index file listing available chunk files (`_index.json`)
#[derive(Deserialize)]
struct ChunkIndex {
    #[allow(dead_code)]
    chunk_size: usize,
    total_bodies: usize,
    chunks: Vec<ChunkEntry>,
}

#[derive(Deserialize)]
struct ChunkEntry {
    name: String,
    #[allow(dead_code)] // present in S3 chunk manifest JSON
    count: usize,
    #[allow(dead_code)]
    start_index: usize,
    #[allow(dead_code)] // present in S3 chunk manifest JSON
    compressed_bytes: usize,
}

/// Stores real email bodies keyed by Proton `remote_id`.
///
/// Unlike `FixtureStore` which uses sequential integer access, this store
/// provides exact lookup by `remote_id` so that each message receives its
/// actual body content.
///
/// Bodies are stored **encrypted** (base64-encoded AES-GCM ciphertext).
/// Decryption happens lazily in `get_body_blocking()` on the caller's thread,
/// which naturally parallelizes across the existing prefetch worker pool.
pub struct RealBodiesStore {
    /// Encrypted bodies keyed by `remote_id` (base64-encoded ciphertext)
    bodies: RwLock<HashMap<String, String>>,
    /// Full list of `remote_ids` from the manifest
    remote_ids: RwLock<Vec<String>>,
    /// Total expected bodies
    expected_total: AtomicUsize,
    /// Whether the manifest has been loaded
    manifest_loaded: AtomicBool,
    /// Whether all loading is complete
    loading_complete: AtomicBool,
    /// Loading error (if any)
    loading_error: RwLock<Option<String>>,
    /// Condition variable for waiting on new bodies
    bodies_available: Condvar,
    /// Mutex for condition variable
    wait_mutex: Mutex<()>,
    /// Counter of bodies served (for stats)
    bodies_served: AtomicUsize,
    /// Accumulated decrypt time in microseconds for served bodies.
    decrypt_micros: AtomicUsize,
    /// Number of successful decrypt operations.
    decrypt_count: AtomicUsize,
    /// Hex-encoded AES-256 key for ciphertext in the store
    encryption_key_hex: String,
}

impl RealBodiesStore {
    fn new(encryption_key_hex: String) -> Self {
        Self {
            bodies: RwLock::new(HashMap::new()),
            remote_ids: RwLock::new(Vec::new()),
            expected_total: AtomicUsize::new(0),
            manifest_loaded: AtomicBool::new(false),
            loading_complete: AtomicBool::new(false),
            loading_error: RwLock::new(None),
            bodies_available: Condvar::new(),
            wait_mutex: Mutex::new(()),
            bodies_served: AtomicUsize::new(0),
            decrypt_micros: AtomicUsize::new(0),
            decrypt_count: AtomicUsize::new(0),
            encryption_key_hex,
        }
    }

    /// Get body for a specific `remote_id`, blocking until available.
    ///
    /// Returns the decrypted body for the given Proton message ID.
    /// The body is stored encrypted; decryption happens here on the caller's
    /// thread, naturally parallelizing across the prefetch worker pool.
    /// Blocks if the body hasn't been fetched yet. Returns an error if
    /// loading completes and the `remote_id` is not found.
    pub fn get_body_blocking(&self, remote_id: &str) -> Result<String, FixtureError> {
        let mut wait_logged = false;

        loop {
            {
                let bodies = acquire_read(&self.bodies);

                // Check if we have this specific body (still encrypted)
                if let Some(encrypted_body) = bodies.get(remote_id) {
                    let encrypted = encrypted_body.clone();
                    drop(bodies); // Release lock before decrypting
                    let decrypt_start = std::time::Instant::now();
                    let decrypted = decrypt_body(&encrypted, &self.encryption_key_hex);
                    let decrypt_elapsed = decrypt_start.elapsed();
                    self.record_decrypt_timing(decrypt_elapsed);
                    self.bodies_served.fetch_add(1, Ordering::Relaxed);
                    return decrypted;
                }

                let is_complete = self.loading_complete.load(Ordering::SeqCst);

                if is_complete {
                    // Check for loading error
                    if let Some(err) = acquire_read(&self.loading_error).as_ref() {
                        return Err(FixtureError::ApiError(err.clone()));
                    }

                    // Loading finished but we don't have this ID
                    let n = bodies.len();
                    return Err(FixtureError::ApiError(format!(
                        "remote_id '{remote_id}' not found in real bodies ({n} bodies available)"
                    )));
                }

                if !wait_logged {
                    let have = bodies.len();
                    let expected = self.expected_total.load(Ordering::SeqCst);
                    debug!("Waiting for body remote_id={remote_id} (have {have}/{expected})");
                    wait_logged = true;
                }
            }

            // Wait for more bodies to be available
            let guard = acquire_mutex(&self.wait_mutex);
            let _guard = match self
                .bodies_available
                .wait_timeout(guard, std::time::Duration::from_millis(100))
            {
                Ok((g, _)) => g,
                Err(e) => std::sync::PoisonError::into_inner(e).0,
            };
        }
    }

    /// Add bodies to the store (called from background loader).
    /// Also records `remote_ids` for stats tracking.
    fn add_bodies_map(&self, new_bodies: Vec<(String, String)>) {
        let count = new_bodies.len();
        let mut ids = acquire_write(&self.remote_ids);
        let mut bodies = acquire_write(&self.bodies);
        for (remote_id, body) in new_bodies {
            ids.push(remote_id.clone());
            bodies.insert(remote_id, body);
        }
        drop(bodies);
        drop(ids);

        let total = self.len();
        debug!("Added {count} real bodies, total now: {total}");
        self.bodies_available.notify_all();
    }

    /// Mark loading as complete
    fn mark_complete(&self) {
        self.loading_complete.store(true, Ordering::SeqCst);
        self.bodies_available.notify_all();
        let n = self.len();
        info!("Real bodies loading complete. Total: {n}");
    }

    /// Mark loading as failed
    fn mark_error(&self, error: String) {
        *acquire_write(&self.loading_error) = Some(error);
        self.loading_complete.store(true, Ordering::SeqCst);
        self.bodies_available.notify_all();
    }

    /// Number of bodies currently loaded
    pub fn len(&self) -> usize {
        acquire_read(&self.bodies).len()
    }

    /// Whether the store is empty
    pub fn is_empty(&self) -> bool {
        acquire_read(&self.bodies).is_empty()
    }

    /// Whether loading is complete
    pub fn is_loading_complete(&self) -> bool {
        self.loading_complete.load(Ordering::SeqCst)
    }

    fn record_decrypt_timing(&self, duration: Duration) {
        let micros = duration.as_micros().min(usize::MAX as u128) as usize;
        self.decrypt_micros.fetch_add(micros, Ordering::Relaxed);
        self.decrypt_count.fetch_add(1, Ordering::Relaxed);
    }
}

/// Fetch the chunk index (`{base_url}/_index.json`)
async fn fetch_chunk_index(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<ChunkIndex, FixtureError> {
    let url = format!("{base_url}/_index.json");
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| FixtureError::ApiError(format!("Failed to fetch _index.json: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(FixtureError::ApiError(format!(
            "_index.json HTTP status: {status}"
        )));
    }

    response
        .json()
        .await
        .map_err(|e| FixtureError::ApiError(format!("_index.json parse error: {e}")))
}

/// Download a chunk file from S3 and decompress it into a map of
/// `remote_id` -> encrypted body (base64). No decryption is done here;
/// that happens lazily on the prefetch worker threads.
async fn fetch_chunk(
    client: &reqwest::Client,
    base_url: &str,
    chunk_name: &str,
) -> Result<HashMap<String, String>, FixtureError> {
    let url = format!("{base_url}/{chunk_name}");

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| FixtureError::ApiError(format!("Failed to fetch {chunk_name}: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(FixtureError::ApiError(format!(
            "{chunk_name} HTTP status: {status}"
        )));
    }

    let compressed_bytes = response
        .bytes()
        .await
        .map_err(|e| FixtureError::ApiError(format!("Failed to read {chunk_name}: {e}")))?;

    let nbytes = compressed_bytes.len();
    info!("Downloaded {chunk_name} ({nbytes} bytes compressed)");

    // Decompress the chunk file (gzip)
    let json_bytes = decompress_gzip(&compressed_bytes)?;

    // Parse JSON: { "remote_id": "encrypted_base64_body", ... }
    let chunk_map: HashMap<String, String> = serde_json::from_slice(&json_bytes)
        .map_err(|e| FixtureError::ApiError(format!("{chunk_name} JSON parse error: {e}")))?;

    let n_bodies = chunk_map.len();
    let n_json = json_bytes.len();
    info!("Parsed {chunk_name} ({n_bodies} bodies, {n_json} bytes JSON)");

    Ok(chunk_map)
}

/// Background loader that downloads ALL chunk files concurrently,
/// then adds encrypted bodies to the store. Decryption is deferred to
/// the prefetch worker threads via `get_body_blocking()`.
fn spawn_chunked_bodies_loader(store: Arc<RealBodiesStore>, base_url: String) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                error!("Failed to create tokio runtime: {e}");
                store.mark_error(format!("Runtime creation failed: {e}"));
                return;
            }
        };

        rt.block_on(async {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_default();

            // Step 1: Fetch the chunk index
            let index = match fetch_chunk_index(&client, &base_url).await {
                Ok(idx) => idx,
                Err(e) => {
                    error!("Failed to fetch chunk index: {e}");
                    store.mark_error(format!("Chunk index fetch failed: {e}"));
                    return;
                }
            };

            let total_bodies = index.total_bodies;
            let num_chunks = index.chunks.len();
            store.expected_total.store(total_bodies, Ordering::SeqCst);
            store.manifest_loaded.store(true, Ordering::SeqCst);

            info!(
                "Chunk index loaded: {total_bodies} bodies in {num_chunks} chunks. Downloading all concurrently..."
            );

            // Step 2: Download ALL chunks concurrently
            let download_start = std::time::Instant::now();

            let futures: Vec<_> = index
                .chunks
                .iter()
                .map(|entry| {
                    let client = client.clone();
                    let base_url = base_url.clone();
                    let name = entry.name.clone();
                    async move {
                        (
                            name.clone(),
                            fetch_chunk(&client, &base_url, &name).await,
                        )
                    }
                })
                .collect();

            let results = futures::future::join_all(futures).await;

            let download_elapsed = download_start.elapsed();
            let download_secs = download_elapsed.as_secs_f64();
            info!("All {num_chunks} chunks downloaded in {download_secs:.1}s");

            // Step 3: Add encrypted bodies to the store (fast — no decryption)
            let mut total_fetched = 0;

            for (chunk_name, result) in results {
                match result {
                    Ok(chunk_map) => {
                        let count = chunk_map.len();
                        total_fetched += count;

                        let bodies: Vec<(String, String)> = chunk_map.into_iter().collect();
                        store.add_bodies_map(bodies);

                        info!(
                            "Loaded {count} encrypted bodies from {chunk_name} (total: {total_fetched}/{total_bodies})"
                        );
                    }
                    Err(e) => {
                        error!("Failed to fetch chunk {chunk_name}: {e}");
                    }
                }
            }

            store.mark_complete();
            let load_secs = download_start.elapsed().as_secs_f64();
            info!(
                "All bodies available: {total_fetched} encrypted bodies loaded in {load_secs:.1}s (decryption deferred to prefetch threads)"
            );
        });
    });
}

// ========================================================================================
// Real Bodies — Public API
// ========================================================================================

/// Initialize the real-bodies store by downloading chunked files over HTTPS.
///
/// This spawns a background thread that:
/// 1. Fetches `{base_url}/_index.json` to discover available chunks
/// 2. Downloads each chunk object named in the index, decompresses gzip, parses JSON
/// 3. Stores **encrypted** bodies; decryption runs in `get_body_for_remote_id()`
///
/// Use `get_body_for_remote_id()` to look up bodies by Proton message ID.
/// The function returns immediately; bodies are loaded progressively in the background.
pub fn initialize_real_bodies_api(config: ChunkedBodiesFixtureConfig) -> Result<(), FixtureError> {
    let base_url = config.base_url.trim_end_matches('/').to_string();
    let store = Arc::new(RealBodiesStore::new(config.encryption_key_hex));
    let store_clone = Arc::clone(&store);

    *acquire_write(&REAL_BODIES_STORE) = Some(store);

    spawn_chunked_bodies_loader(store_clone, base_url);

    info!("Initialized chunked real-bodies loading (HTTP)");
    Ok(())
}

/// Clear the global real-bodies store so body substitution uses the live mail API again.
///
/// In-flight download tasks may keep their `Arc` until they finish, but
/// [`is_real_bodies_initialized`] becomes false immediately, so perf body substitution no longer
/// serves chunked bodies.
pub fn shutdown_real_bodies_api() {
    *acquire_write(&REAL_BODIES_STORE) = None;
    info!("Shut down chunked real-bodies store");
}

/// Check if the real-bodies store is initialized
#[must_use]
pub fn is_real_bodies_initialized() -> bool {
    acquire_read(&REAL_BODIES_STORE).is_some()
}

/// Check if real-bodies loading is complete
#[must_use]
pub fn is_real_bodies_loading_complete() -> bool {
    acquire_read(&REAL_BODIES_STORE)
        .as_ref()
        .is_some_and(|s| s.is_loading_complete())
}

/// Get the body for a specific Proton remote message ID.
///
/// This blocks until the body is available (if the background loader hasn't
/// reached it yet) or returns an error if loading completes without the ID.
pub fn get_body_for_remote_id(remote_id: &str) -> Result<String, FixtureError> {
    let store = acquire_read(&REAL_BODIES_STORE)
        .as_ref()
        .cloned()
        .ok_or(FixtureError::NotInitialized)?;

    store.get_body_blocking(remote_id)
}

/// Get the number of real bodies currently loaded
#[must_use]
pub fn real_bodies_loaded() -> usize {
    acquire_read(&REAL_BODIES_STORE)
        .as_ref()
        .map_or(0, |s| s.len())
}

/// Statistics for the real-bodies store
pub struct RealBodiesStats {
    pub total_available: usize,
    pub total_fetched: usize,
    pub bodies_served: usize,
    pub loading_complete: bool,
    pub manifest_loaded: bool,
    pub decrypt_total: Duration,
    pub decrypt_count: usize,
}

impl RealBodiesStats {
    #[must_use]
    pub fn snapshot() -> Option<Self> {
        let store = acquire_read(&REAL_BODIES_STORE).as_ref()?.clone();
        Some(Self {
            total_available: store.expected_total.load(Ordering::SeqCst),
            total_fetched: store.len(),
            bodies_served: store.bodies_served.load(Ordering::Relaxed),
            loading_complete: store.is_loading_complete(),
            manifest_loaded: store.manifest_loaded.load(Ordering::SeqCst),
            decrypt_total: Duration::from_micros(
                store.decrypt_micros.load(Ordering::Relaxed) as u64
            ),
            decrypt_count: store.decrypt_count.load(Ordering::Relaxed),
        })
    }
}

impl std::fmt::Display for RealBodiesStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bs = self.bodies_served;
        let tf = self.total_fetched;
        let ta = self.total_available;
        let manifest = if self.manifest_loaded {
            "loaded"
        } else {
            "pending"
        };
        let lc = self.loading_complete;
        let avg_decrypt_ms = if self.decrypt_count == 0 {
            0.0
        } else {
            (self.decrypt_total.as_secs_f64() * 1000.0) / self.decrypt_count as f64
        };
        writeln!(
            f,
            "Real Bodies Stats [API]: {bs} bodies served, {tf}/{ta} fetched (manifest: {manifest}, complete: {lc})"
        )?;
        write!(
            f,
            "  Real bodies decrypt (lazy, on-demand): {:.2}s total ({} decrypts, avg {:.2}ms)",
            self.decrypt_total.as_secs_f64(),
            self.decrypt_count,
            avg_decrypt_ms
        )
    }
}
