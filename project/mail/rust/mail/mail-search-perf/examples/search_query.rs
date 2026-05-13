//! Standalone search query tool for persisted ephemeral DBs.
//!
//! Loads `search_index_blobs` from a user DB and runs a Foundation Search query.
//! Optionally logs in to the Proton API to fetch + decrypt message details for each hit.
//!
//! Offline (no login):
//!   cargo run -p mail-search-perf --example search_query --features foundation_search \
//!     -- --db-path historic_load_test_db/user/<id>.db --query "Youngsters"
//!
//! With server fetch (shows subject + body snippet):
//!   cargo run -p mail-search-perf --example search_query --features foundation_search \
//!     -- --db-path historic_load_test_db/user/<id>.db --query "Youngsters" \
//!     --username <email> --password <pass>

#[path = "historic_load/core.rs"]
mod historic_load_core;

use std::collections::HashMap;
use std::io::Read as _;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use clap::Parser;
use flate2::read::GzDecoder;
use mail_search::FoundationSearchEngine;
use mail_search::SearchError;
use mail_search::traits::BlobStorage;
use mail_stash::rusqlite;
use tempfile::TempDir;

#[derive(Clone)]
struct ReadOnlyBlobStorage {
    blobs: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

#[async_trait::async_trait]
impl BlobStorage for ReadOnlyBlobStorage {
    async fn load(&self, name: &str) -> Result<Option<Vec<u8>>, SearchError> {
        let blobs = self.blobs.read().unwrap();
        Ok(blobs.get(name).cloned())
    }
    async fn save(&self, _name: &str, _data: &[u8]) -> Result<(), SearchError> {
        Ok(())
    }
    async fn delete(&self, _name: &str) -> Result<bool, SearchError> {
        Ok(false)
    }
    async fn clear_all(&self) -> Result<(), SearchError> {
        Ok(())
    }
}

#[derive(Parser, Debug)]
#[command(
    about = "Search a persisted Foundation Search index, optionally fetch + decrypt hits from server"
)]
struct Args {
    /// Path to the persisted user .db file
    #[clap(long)]
    db_path: PathBuf,
    /// Search query (e.g. "Youngsters")
    #[clap(long)]
    query: String,
    /// Max results to display (default: 20)
    #[clap(long, default_value = "20")]
    limit: usize,
    /// Proton username (enables server fetch for message details)
    #[clap(long)]
    username: Option<String>,
    /// Proton password
    #[clap(long)]
    password: Option<String>,
    /// Proton mailbox password (if different from login password)
    #[clap(long)]
    email_password: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::filter::LevelFilter;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .init();

    let args = Args::parse();

    // --- Load blobs from DB ---
    println!("Opening DB: {:?}", args.db_path);
    let conn = rusqlite::Connection::open_with_flags(
        &args.db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;

    let mut stmt = conn.prepare("SELECT blob_name, blob_data FROM search_index_blobs")?;
    let mut blobs = HashMap::new();
    let mut total_bytes = 0usize;
    let rows = stmt.query_map([], |row| {
        let name: String = row.get(0)?;
        let data: Vec<u8> = row.get(1)?;
        Ok((name, data))
    })?;
    for row in rows {
        let (name, raw) = row?;
        let data = if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
            let mut decoder = GzDecoder::new(&raw[..]);
            let mut out = Vec::new();
            decoder.read_to_end(&mut out)?;
            out
        } else {
            raw
        };
        total_bytes += data.len();
        blobs.insert(name, data);
    }
    drop(stmt);
    drop(conn);

    println!(
        "Loaded {} blobs ({:.2} MB) from search_index_blobs\n",
        blobs.len(),
        total_bytes as f64 / (1024.0 * 1024.0)
    );

    // --- Build engine and search ---
    let storage = ReadOnlyBlobStorage {
        blobs: Arc::new(RwLock::new(blobs)),
    };
    let task_service = Arc::new(
        mail_task_service::TaskService::new(tokio::runtime::Handle::current())
            .expect("TaskService"),
    );
    let engine = FoundationSearchEngine::new(storage, task_service);

    println!("Searching for: {:?}", args.query);
    let search_start = Instant::now();
    let results = engine.search_with_metadata(&args.query).await?;
    let search_elapsed = search_start.elapsed();

    println!(
        "Found {} results in {:.1}ms\n",
        results.len(),
        search_elapsed.as_secs_f64() * 1000.0
    );

    // --- Optional: login for server fetch ---
    // Keep _tmp_dir alive so the temp DB isn't deleted while we use the session.
    let (_tmp_dir, user_ctx) =
        if let (Some(username), Some(password)) = (args.username, args.password) {
            println!("Logging in to fetch message details...");
            let tmp = TempDir::new()?;
            let mail_ctx = historic_load_core::new_mail_context(&tmp).await?;
            let uctx = historic_load_core::login_and_user_context(
                &mail_ctx,
                username,
                password,
                args.email_password,
            )
            .await?;
            println!(
                "Logged in. Fetching {} messages...\n",
                results.len().min(args.limit)
            );
            (Some(tmp), Some(uctx))
        } else {
            (None, None)
        };

    // --- Display results ---
    for (i, entry) in results.iter().enumerate().take(args.limit) {
        let remote_id = entry.identifier().to_string();

        println!("  [{}] id={}", i + 1, remote_id);
        println!("       score={:.4}", entry.score());

        for m in entry.matches() {
            for occ in m.occurrences() {
                println!(
                    "       match: field={:?} pos={} idx={}",
                    occ.attribute(),
                    occ.position().0,
                    occ.index().0,
                );
            }
        }

        if let Some(ref user_ctx) = user_ctx {
            use mail_api::services::proton::{ProtonMail, common::MessageId};
            use mail_crypto_inbox::message::{DecryptableMessage as _, DecryptedBody};
            use mail_crypto_inbox::proton_crypto;
            use mail_crypto_inbox::proton_crypto_account::keys::AddressKeySelector;
            use mail_html_transformer::html_to_text_fast;

            let session = user_ctx.session();
            let mid = MessageId::from(remote_id.clone());

            match ProtonMail::get_message(session, mid).await {
                Ok(resp) => {
                    let meta = &resp.message.metadata;
                    println!("       subject: {}", meta.subject);
                    println!("       from: {}", meta.sender.address.as_clear_text_str());
                    println!(
                        "       date: {}",
                        chrono::DateTime::from_timestamp(meta.time as i64, 0)
                            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                            .unwrap_or_else(|| meta.time.to_string())
                    );

                    let pgp = proton_crypto::new_pgp_provider();
                    let address_id = meta.address_id.clone();

                    let encrypted = mail_common::datatypes::EncryptedMessageBody {
                        encrypted_body: resp.message.body.body,
                        metadata: mail_common::models::MessageBodyMetadata {
                            remote_message_id: Some(MessageId::from(remote_id.clone())),
                            mime_type: resp.message.body.mime_type.into(),
                            ..Default::default()
                        },
                    };

                    let tether = user_ctx.user_stash().connection();
                    let snippet = match user_ctx
                        .crypto_key_service()
                        .load_with_tether(user_ctx.user_context(), &tether)
                        .address_keys(&pgp, &address_id)
                        .await
                        .map(AddressKeySelector::into_raw_keys)
                    {
                        Ok(keys) => match encrypted.decrypt(&pgp, &keys) {
                            Ok(raw) => match raw.processed_body() {
                                Ok(body) => {
                                    let text = match &body {
                                        DecryptedBody::Plain(t) => t.clone(),
                                        _ => html_to_text_fast(body.body()),
                                    };
                                    let chars: String = text.chars().take(200).collect();
                                    chars
                                }
                                Err(e) => format!("[body processing error: {e}]"),
                            },
                            Err(e) => format!("[decrypt error: {e}]"),
                        },
                        Err(e) => format!("[key error: {e}]"),
                    };
                    println!("       body: {}", snippet.replace('\n', " "));
                }
                Err(e) => {
                    println!("       [fetch failed: {e}]");
                }
            }
        }

        println!();
    }
    if results.len() > args.limit {
        println!("  ... and {} more", results.len() - args.limit);
    }

    Ok(())
}
