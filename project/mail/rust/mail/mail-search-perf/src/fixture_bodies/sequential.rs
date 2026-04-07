//! Sequential JSONL + integer batch-API fixture bodies (`initialize`, `initialize_from_api`).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};

use serde::Deserialize;
use tracing::{debug, error, info, warn};

use super::BatchApiFixtureConfig;
use super::FixtureError;
use super::crypto::decrypt_body;
use super::util::{acquire_mutex, acquire_read, acquire_write};
use crate::DeclaredFixtureMime;
use crate::SubstituteBody;

const API_BATCH_SIZE: usize = 100; // API max is 100 IDs per request

static FIXTURE_STORE: RwLock<Option<Arc<FixtureStore>>> = RwLock::new(None);

/// Counter for sequential body retrieval (fixture mode only)
static BODY_INDEX: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug)]
struct FixtureBodySlot {
    body: String,
    mime: DeclaredFixtureMime,
}

/// Map optional `mime` from fixture JSON/API (declared by the source, never inferred from bytes).
fn declared_mime_from_fixture_field(mime: Option<&str>) -> DeclaredFixtureMime {
    let Some(raw) = mime.map(str::trim) else {
        return DeclaredFixtureMime::TextHtml;
    };
    if raw.is_empty() {
        return DeclaredFixtureMime::TextHtml;
    }
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("text/plain") {
        DeclaredFixtureMime::TextPlain
    } else {
        DeclaredFixtureMime::TextHtml
    }
}

/// A single email entry from the fixture file
#[derive(Debug, Deserialize)]
pub struct FixtureEmail {
    pub id: String,
    pub subject: String,
    pub body: String,
    /// Declared MIME (e.g. `"text/html"` / `"text/plain"`). Defaults to HTML when omitted.
    #[serde(default)]
    pub mime: Option<String>,
    #[serde(default)]
    pub sender: Option<serde_json::Value>,
    #[serde(default)]
    pub to: Option<serde_json::Value>,
    #[serde(default)]
    pub cc: Option<serde_json::Value>,
    #[serde(default)]
    pub bcc: Option<serde_json::Value>,
    #[serde(default)]
    pub time: Option<i64>,
}

/// API request structure
#[derive(serde::Serialize)]
struct ApiRequest {
    ids: Vec<u32>,
}

/// API response structures
#[derive(Deserialize)]
struct ApiResponse {
    bodies: Vec<BodyItem>,
    #[serde(default)]
    errors: Vec<ErrorItem>,
}

#[derive(Deserialize)]
struct BodyItem {
    id: u32,
    body: String,
    #[serde(default)]
    mime: Option<String>,
}

#[derive(Deserialize)]
struct ErrorItem {
    id: u32,
    error: String,
}

/// Stores loaded fixture bodies with support for progressive loading
pub struct FixtureStore {
    /// Pre-loaded bodies and declared MIME from the fixture source.
    bodies: RwLock<Vec<FixtureBodySlot>>,
    /// Total expected bodies (for API mode)
    expected_total: AtomicUsize,
    /// Whether loading is complete
    loading_complete: AtomicBool,
    /// Whether an error occurred during loading
    loading_error: RwLock<Option<String>>,
    /// Condition variable for waiting on new bodies
    bodies_available: Condvar,
    /// Mutex for condition variable
    wait_mutex: Mutex<()>,
    /// Source type for stats
    source: RwLock<FixtureSource>,
}

#[derive(Clone, Debug)]
pub enum FixtureSource {
    File(String),
    Api { total_requested: usize },
    RealApi { total_available: usize },
    None,
}

impl FixtureStore {
    /// Create a new empty store for progressive loading
    fn new_for_api(expected_total: usize) -> Self {
        Self {
            bodies: RwLock::new(Vec::with_capacity(expected_total)),
            expected_total: AtomicUsize::new(expected_total),
            loading_complete: AtomicBool::new(false),
            loading_error: RwLock::new(None),
            bodies_available: Condvar::new(),
            wait_mutex: Mutex::new(()),
            source: RwLock::new(FixtureSource::Api {
                total_requested: expected_total,
            }),
        }
    }

    /// Load fixtures from a JSONL file (original behavior)
    pub fn load_from_file(path: &Path) -> Result<Self, FixtureError> {
        let file = File::open(path).map_err(|e| FixtureError::IoError(e.to_string()))?;
        let reader = BufReader::new(file);

        let mut bodies = Vec::new();
        let mut line_num = 0;

        for line in reader.lines() {
            line_num += 1;
            let line = line.map_err(|e| FixtureError::IoError(e.to_string()))?;

            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<FixtureEmail>(&line) {
                Ok(email) => {
                    bodies.push(FixtureBodySlot {
                        body: email.body,
                        mime: declared_mime_from_fixture_field(email.mime.as_deref()),
                    });
                }
                Err(e) => {
                    warn!("Failed to parse fixture line {line_num}: {e}");
                }
            }
        }

        let n = bodies.len();
        let path_disp = path.display();
        info!("Loaded {n} fixture bodies from {path_disp}");

        if bodies.is_empty() {
            return Err(FixtureError::EmptyFixture);
        }

        let len = bodies.len();
        Ok(Self {
            bodies: RwLock::new(bodies),
            expected_total: AtomicUsize::new(len),
            loading_complete: AtomicBool::new(true),
            loading_error: RwLock::new(None),
            bodies_available: Condvar::new(),
            wait_mutex: Mutex::new(()),
            source: RwLock::new(FixtureSource::File(path.display().to_string())),
        })
    }

    /// Get the number of currently loaded bodies
    pub fn len(&self) -> usize {
        acquire_read(&self.bodies).len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        acquire_read(&self.bodies).is_empty()
    }

    /// Check if loading is complete
    pub fn is_loading_complete(&self) -> bool {
        self.loading_complete.load(Ordering::SeqCst)
    }

    /// Get body at a specific index, blocking if not yet available
    ///
    /// For API-based loading, this will wait until the body at `index` is actually
    /// fetched from the API. Only wraps around once loading is complete.
    fn get_body_blocking(&self, index: usize) -> Result<FixtureBodySlot, FixtureError> {
        let mut wait_logged = false;

        loop {
            // Check if we have the requested body
            {
                let bodies = acquire_read(&self.bodies);
                let len = bodies.len();
                let is_complete = self.loading_complete.load(Ordering::SeqCst);

                // If loading is complete, we can wrap around
                if is_complete {
                    if len == 0 {
                        // Check for error
                        if let Some(err) = acquire_read(&self.loading_error).as_ref() {
                            return Err(FixtureError::ApiError(err.clone()));
                        }
                        return Err(FixtureError::EmptyFixture);
                    }
                    // Wrap around since loading is done
                    return Ok(bodies[index % len].clone());
                }

                // Loading still in progress - only return if we have this specific index
                if index < len {
                    return Ok(bodies[index].clone());
                }

                // Log when we start waiting (only once per request)
                if !wait_logged {
                    let expected = self.expected_total.load(Ordering::SeqCst);
                    debug!("Waiting for body #{index} (currently have {len}/{expected})");
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

    /// Add bodies (called from background loader)
    fn add_bodies(&self, new_bodies: Vec<FixtureBodySlot>) {
        let mut bodies = acquire_write(&self.bodies);
        let count = new_bodies.len();
        bodies.extend(new_bodies);
        drop(bodies);

        let total = self.len();
        debug!("Added {count} bodies, total now: {total}");

        // Notify waiters
        self.bodies_available.notify_all();
    }

    /// Mark loading as complete
    fn mark_complete(&self) {
        self.loading_complete.store(true, Ordering::SeqCst);
        self.bodies_available.notify_all();
        let n = self.len();
        info!("Fixture loading complete. Total bodies: {n}");
    }

    /// Mark loading as failed
    fn mark_error(&self, error: String) {
        *acquire_write(&self.loading_error) = Some(error);
        self.loading_complete.store(true, Ordering::SeqCst);
        self.bodies_available.notify_all();
    }

    /// Get the source type
    pub fn source(&self) -> FixtureSource {
        acquire_read(&self.source).clone()
    }
}
/// Fetch a batch of bodies from the API
async fn fetch_batch(
    client: &reqwest::Client,
    api_url: &str,
    encryption_key_hex: &str,
    ids: Vec<u32>,
) -> Result<Vec<(u32, FixtureBodySlot)>, FixtureError> {
    let response = client
        .post(api_url)
        .json(&ApiRequest { ids: ids.clone() })
        .send()
        .await
        .map_err(|e| FixtureError::ApiError(format!("HTTP error: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(FixtureError::ApiError(format!("HTTP status: {status}")));
    }

    let api_response: ApiResponse = response
        .json()
        .await
        .map_err(|e| FixtureError::ApiError(format!("JSON parse error: {e}")))?;

    // Log any errors
    for err in &api_response.errors {
        let id = err.id;
        let msg = &err.error;
        debug!("API error for ID {id}: {msg}");
    }

    // Decrypt all bodies
    let mut results = Vec::with_capacity(api_response.bodies.len());
    for item in api_response.bodies {
        match decrypt_body(&item.body, encryption_key_hex) {
            Ok(decrypted) => results.push((
                item.id,
                FixtureBodySlot {
                    body: decrypted,
                    mime: declared_mime_from_fixture_field(item.mime.as_deref()),
                },
            )),
            Err(e) => {
                let id = item.id;
                warn!("Failed to decrypt body {id}: {e}");
            }
        }
    }

    Ok(results)
}

/// Background loader that fetches bodies from the API
fn spawn_api_loader(
    store: Arc<FixtureStore>,
    total_count: usize,
    concurrent_batches: usize,
    config: BatchApiFixtureConfig,
) {
    std::thread::spawn(move || {
        // Create a tokio runtime for async operations
        let rt = match tokio::runtime::Builder::new_current_thread()
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
            let api_url = config.api_url.clone();
            let encryption_key_hex = config.encryption_key_hex.clone();
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default();

            if u32::try_from(total_count).is_err() {
                let max_id = u32::MAX;
                store.mark_error(format!(
                    "Fixture batch API total_count ({total_count}) exceeds maximum request id ({max_id})"
                ));
                return;
            }

            // Calculate batch ranges
            let mut batch_starts: Vec<usize> = (0..total_count).step_by(API_BATCH_SIZE).collect();
            let total_batches = batch_starts.len();

            info!(
                "Starting API fetch: {total_count} bodies in {total_batches} batches ({concurrent_batches} concurrent)"
            );

            let mut completed_batches = 0;
            let mut total_fetched = 0;

            // Process batches with limited concurrency
            while !batch_starts.is_empty() {
                let batch_chunk: Vec<usize> = batch_starts
                    .drain(..batch_starts.len().min(concurrent_batches))
                    .collect();

                // Fetch batches concurrently
                let futures: Vec<_> = batch_chunk
                    .into_iter()
                    .map(|start| {
                        let client = client.clone();
                        let api_url = api_url.clone();
                        let encryption_key_hex = encryption_key_hex.clone();
                        let end = (start + API_BATCH_SIZE).min(total_count);
                        // `total_count` fits in u32 (checked above), so every index in this range does too.
                        #[allow(clippy::cast_possible_truncation)]
                        let ids: Vec<u32> = (start..end).map(|i| i as u32).collect();
                        async move {
                            (
                                start,
                                fetch_batch(&client, &api_url, &encryption_key_hex, ids).await,
                            )
                        }
                    })
                    .collect();

                let results = futures::future::join_all(futures).await;

                // Process results and add to store
                for (batch_start, result) in results {
                    completed_batches += 1;
                    match result {
                        Ok(bodies) => {
                            let count = bodies.len();
                            total_fetched += count;

                            // Sort by ID to maintain order
                            let mut sorted: Vec<_> = bodies;
                            sorted.sort_by_key(|(id, _)| *id);
                            let slots: Vec<FixtureBodySlot> =
                                sorted.into_iter().map(|(_, slot)| slot).collect();

                            store.add_bodies(slots);

                            if completed_batches % 10 == 0 || completed_batches == total_batches {
                                info!(
                                    "API fetch progress: {completed_batches}/{total_batches} batches, {total_fetched} bodies"
                                );
                            }
                        }
                        Err(e) => {
                            warn!("Batch starting at {batch_start} failed: {e}. Continuing...");
                        }
                    }
                }
            }

            store.mark_complete();
            info!("API fetch complete: {total_fetched} bodies fetched from {total_batches} batches");
        });
    });
}

/// Initialize the fixture store from a file path.
///
/// This should be called once before running historic load.
/// The path can be provided directly or via the `FIXTURE_BODIES_PATH` env var.
/// If already initialized, this will reset and reinitialize.
pub fn initialize(path: Option<&Path>) -> Result<(), FixtureError> {
    let path = path
        .map(Path::to_path_buf)
        .or_else(|| std::env::var("FIXTURE_BODIES_PATH").ok().map(Into::into))
        .ok_or_else(|| {
            FixtureError::IoError(
                "No fixture path provided and FIXTURE_BODIES_PATH not set".to_string(),
            )
        })?;

    let store = Arc::new(FixtureStore::load_from_file(&path)?);

    // Reset the body index
    BODY_INDEX.store(0, Ordering::SeqCst);

    // Set the new store (replacing any existing one)
    *acquire_write(&FIXTURE_STORE) = Some(store);

    Ok(())
}

/// Initialize the fixture store from the remote API with progressive loading.
///
/// This spawns a background thread that fetches bodies in batches.
/// The `get_next_body()` function will block if indexing outruns the fetcher.
/// If already initialized, this will reset and reinitialize.
///
/// # Arguments
/// * `config` - POST URL and hex key for your fixture deployment ([`BatchApiFixtureConfig`])
/// * `total_count` - Total number of email IDs to fetch (0 to total_count-1)
/// * `concurrent_batches` - Number of concurrent API requests (recommended: 5-10)
///
/// # Returns
/// Ok(()) immediately after starting the background loader
pub fn initialize_from_api(
    config: BatchApiFixtureConfig,
    total_count: usize,
    concurrent_batches: usize,
) -> Result<(), FixtureError> {
    if total_count == 0 {
        return Err(FixtureError::ApiError(
            "total_count must be > 0".to_string(),
        ));
    }

    let store = Arc::new(FixtureStore::new_for_api(total_count));
    let store_clone = Arc::clone(&store);

    // Reset the body index
    BODY_INDEX.store(0, Ordering::SeqCst);

    // Set the new store (replacing any existing one)
    *acquire_write(&FIXTURE_STORE) = Some(store);

    // Spawn background loader
    spawn_api_loader(store_clone, total_count, concurrent_batches.max(1), config);

    info!("Initialized API-based fixture loading for {total_count} bodies");

    Ok(())
}

/// Check if fixtures are initialized
#[must_use]
pub fn is_initialized() -> bool {
    acquire_read(&FIXTURE_STORE).is_some()
}

/// Check if fixture loading is complete (always true for file-based)
#[must_use]
pub fn is_loading_complete() -> bool {
    acquire_read(&FIXTURE_STORE)
        .as_ref()
        .is_some_and(|s| s.is_loading_complete())
}

/// Get the next body from the fixture store (sequential access).
///
/// This increments an internal counter and returns bodies in order,
/// wrapping around when the fixture is exhausted.
///
/// For API-based loading, this will block if the requested body
/// hasn't been fetched yet.
pub fn get_next_body() -> Result<String, FixtureError> {
    Ok(get_next_substitute_body()?.body)
}

/// Next sequential fixture body including declared MIME from the fixture source.
pub fn get_next_substitute_body() -> Result<SubstituteBody, FixtureError> {
    let store = acquire_read(&FIXTURE_STORE)
        .as_ref()
        .cloned()
        .ok_or(FixtureError::NotInitialized)?;

    let index = BODY_INDEX.fetch_add(1, Ordering::SeqCst);
    let slot = store.get_body_blocking(index)?;

    if index < 10 || index.is_multiple_of(1000) {
        let nbytes = slot.body.len();
        debug!("Fixture body #{index}: {nbytes} bytes");
    }

    Ok(SubstituteBody {
        body: slot.body,
        mime: slot.mime,
    })
}

/// Reset the body index counter (for testing multiple runs)
pub fn reset_index() {
    BODY_INDEX.store(0, Ordering::SeqCst);
    info!("Fixture body index reset to 0");
}

/// Get the current index (for debugging)
pub fn current_index() -> usize {
    BODY_INDEX.load(Ordering::SeqCst)
}

/// Get total number of loaded fixtures (returns 0 if not initialized)
#[must_use]
pub fn total_fixtures() -> usize {
    acquire_read(&FIXTURE_STORE).as_ref().map_or(0, |s| s.len())
}

/// Get the expected total (for API mode, this is the requested count)
#[must_use]
pub fn expected_total() -> usize {
    acquire_read(&FIXTURE_STORE)
        .as_ref()
        .map_or(0, |s| s.expected_total.load(Ordering::SeqCst))
}

/// Statistics about fixture usage
pub struct FixtureStats {
    pub source: FixtureSource,
    pub total_fixtures: usize,
    pub expected_total: usize,
    pub bodies_served: usize,
    pub wrap_count: usize,
    pub loading_complete: bool,
}

impl FixtureStats {
    #[must_use]
    pub fn snapshot() -> Option<Self> {
        let store = acquire_read(&FIXTURE_STORE).as_ref()?.clone();
        let bodies_served = BODY_INDEX.load(Ordering::SeqCst);
        let total = store.len();
        let expected = store.expected_total.load(Ordering::SeqCst);
        let wrap_count = if total > 0 { bodies_served / total } else { 0 };

        Some(Self {
            source: store.source(),
            total_fixtures: total,
            expected_total: expected,
            bodies_served,
            wrap_count,
            loading_complete: store.is_loading_complete(),
        })
    }
}

impl std::fmt::Display for FixtureStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let source_str = match &self.source {
            FixtureSource::File(path) => format!("file: {path}"),
            FixtureSource::Api { total_requested } => {
                format!("API ({total_requested} requested)")
            }
            FixtureSource::RealApi { total_available } => {
                format!("Real API ({total_available} available)")
            }
            FixtureSource::None => "none".to_string(),
        };

        let bs = self.bodies_served;
        let tf = self.total_fixtures;
        let et = self.expected_total;
        let wc = self.wrap_count;
        let lc = self.loading_complete;
        write!(
            f,
            "Fixture Stats [{source_str}]: {bs} bodies served from {tf}/{et} loaded ({wc} wraps, complete: {lc})"
        )
    }
}

/// Wait for at least `min_bodies` to be loaded (useful for ensuring data before starting)
pub fn wait_for_bodies(
    min_bodies: usize,
    timeout: std::time::Duration,
) -> Result<usize, FixtureError> {
    let store = acquire_read(&FIXTURE_STORE)
        .as_ref()
        .cloned()
        .ok_or(FixtureError::NotInitialized)?;
    let start = std::time::Instant::now();

    loop {
        let current = store.len();
        if current >= min_bodies || store.is_loading_complete() {
            return Ok(current);
        }

        if start.elapsed() > timeout {
            return Err(FixtureError::ApiError(format!(
                "Timeout waiting for {min_bodies} bodies (got {current})",
            )));
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
