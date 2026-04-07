//! Mint `LocalMessageId` values (see `mail_common::datatypes`) in a reserved high
//! range for historic-load / perf ingest before relying on `SQLite` autoincrement.
//!
//! For normal app ingest, local ids come from the DB; this is only for this crate’s scenarios.

use mail_common::datatypes::LocalMessageId;
use std::sync::atomic::{AtomicU64, Ordering};

/// Lowest u64 for lab-minted local message ids (avoids colliding with normal `SQLite` ids).
pub const SYNTHETIC_LOCAL_MESSAGE_ID_MIN: u64 = 1 << 60;

/// In-process ids for historic / perf ingest (not normal autoincrement-backed messages).
#[derive(Debug)]
pub struct SyntheticLocalMessageIdSeq {
    next: AtomicU64,
}

impl Default for SyntheticLocalMessageIdSeq {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntheticLocalMessageIdSeq {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next: AtomicU64::new(SYNTHETIC_LOCAL_MESSAGE_ID_MIN),
        }
    }

    /// Next synthetic id. Panics if exhausted (very unlikely).
    pub fn next_id(&self) -> LocalMessageId {
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        assert!(
            n < u64::MAX - 1,
            "synthetic local message id space exhausted"
        );
        LocalMessageId::from(n)
    }
}
