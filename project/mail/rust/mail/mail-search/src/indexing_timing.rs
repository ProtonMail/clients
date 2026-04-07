//! Timing instrumentation for indexing operations.
//!
//! This module provides atomic counters for measuring how much time is spent
//! in various phases of the search indexing process. Enable with `mail-common`'s `foundation_search_index_timing` feature.
//!
//! Usage:
//! ```bash
//! cargo run -p mail-search-perf --example historic_load_test --features foundation_search,foundation_search_index_timing
//! ```
//!
//! Create a `BatchStopwatch` at batch start with `start()`, then call `record_prep_done`,
//! `record_index_done`, `record_cleanup_done` at the end of each phase. Finish with
//! `record_batch_complete`. The stopwatch stores phase start times internally.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Accumulated time spent preparing messages for indexing (DB reads, HTML conversion)
static PREP_TIME_MICROS: AtomicU64 = AtomicU64::new(0);

/// Accumulated time spent in Foundation Search indexing (tokenization, index writes)
static INDEX_TIME_MICROS: AtomicU64 = AtomicU64::new(0);

/// Accumulated time spent in cleanup (deleting intents, saving content hashes)
static CLEANUP_TIME_MICROS: AtomicU64 = AtomicU64::new(0);

/// Total number of messages indexed
static TOTAL_MESSAGES_INDEXED: AtomicU64 = AtomicU64::new(0);

/// Total number of batches processed
static TOTAL_BATCHES: AtomicU64 = AtomicU64::new(0);

fn add_micros(target: &AtomicU64, duration: Duration) {
    let micros = u64::try_from(duration.as_micros().min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
    target.fetch_add(micros, Ordering::Relaxed);
}

/// Stopwatch for a single indexing batch.
///
/// Create at batch start with `BatchStopwatch::start()`, then call `record_prep_done`,
/// `record_index_done`, `record_cleanup_done` at the end of each phase. Finish with
/// `record_batch_complete` which consumes `self`. Methods return `Self` for chaining.
///
/// The stopwatch stores the phase start `Instant` internally, so you don't pass timestamps.
#[derive(Debug)]
pub struct BatchStopwatch {
    phase_start: Instant,
}

impl BatchStopwatch {
    /// Start timing a batch. Call when the batch (or first phase) begins.
    #[must_use]
    pub fn start() -> Self {
        Self {
            phase_start: Instant::now(),
        }
    }

    /// Record preparation phase complete. Call at end of prep; returns `Self` for chaining.
    #[must_use]
    pub fn record_prep_done(self) -> Self {
        add_micros(&PREP_TIME_MICROS, self.phase_start.elapsed());
        Self {
            phase_start: Instant::now(),
        }
    }

    /// Record Foundation Search indexing phase complete. Call at end of indexing.
    #[must_use]
    pub fn record_index_done(self) -> Self {
        add_micros(&INDEX_TIME_MICROS, self.phase_start.elapsed());
        Self {
            phase_start: Instant::now(),
        }
    }

    /// Record cleanup phase complete. Call at end of cleanup.
    #[must_use]
    pub fn record_cleanup_done(self) -> Self {
        add_micros(&CLEANUP_TIME_MICROS, self.phase_start.elapsed());
        Self {
            phase_start: Instant::now(),
        }
    }

    /// Record batch completion with message count. Consumes `self`.
    pub fn record_batch_complete(self, message_count: usize) {
        TOTAL_MESSAGES_INDEXED.fetch_add(message_count as u64, Ordering::Relaxed);
        TOTAL_BATCHES.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for BatchStopwatch {
    fn default() -> Self {
        Self::start()
    }
}

/// Reset all counters (call at start of test)
pub fn reset() {
    PREP_TIME_MICROS.store(0, Ordering::Relaxed);
    INDEX_TIME_MICROS.store(0, Ordering::Relaxed);
    CLEANUP_TIME_MICROS.store(0, Ordering::Relaxed);
    TOTAL_MESSAGES_INDEXED.store(0, Ordering::Relaxed);
    TOTAL_BATCHES.store(0, Ordering::Relaxed);
}

/// Statistics snapshot for display
#[derive(Debug, Clone)]
pub struct IndexingTimingStats {
    pub prep_time: Duration,
    pub index_time: Duration,
    pub cleanup_time: Duration,
    pub total_messages: u64,
    pub total_batches: u64,
}

impl IndexingTimingStats {
    /// Take a snapshot of current timing statistics
    pub fn snapshot() -> Self {
        Self {
            prep_time: Duration::from_micros(PREP_TIME_MICROS.load(Ordering::Relaxed)),
            index_time: Duration::from_micros(INDEX_TIME_MICROS.load(Ordering::Relaxed)),
            cleanup_time: Duration::from_micros(CLEANUP_TIME_MICROS.load(Ordering::Relaxed)),
            total_messages: TOTAL_MESSAGES_INDEXED.load(Ordering::Relaxed),
            total_batches: TOTAL_BATCHES.load(Ordering::Relaxed),
        }
    }

    /// Total measured time (prep + index + cleanup)
    #[must_use]
    pub fn total_time(&self) -> Duration {
        self.prep_time + self.index_time + self.cleanup_time
    }

    /// Average prep time per message
    #[must_use]
    pub fn avg_prep_per_message(&self) -> Duration {
        if self.total_messages == 0 {
            Duration::ZERO
        } else {
            let divisor =
                u32::try_from(self.total_messages.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            self.prep_time / divisor
        }
    }

    /// Average index time per message
    #[must_use]
    pub fn avg_index_per_message(&self) -> Duration {
        if self.total_messages == 0 {
            Duration::ZERO
        } else {
            let divisor =
                u32::try_from(self.total_messages.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            self.index_time / divisor
        }
    }

    /// Average cleanup time per message
    #[must_use]
    pub fn avg_cleanup_per_message(&self) -> Duration {
        if self.total_messages == 0 {
            Duration::ZERO
        } else {
            let divisor =
                u32::try_from(self.total_messages.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            self.cleanup_time / divisor
        }
    }

    /// Average batch size
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_batch_size(&self) -> f64 {
        if self.total_batches == 0 {
            0.0
        } else {
            self.total_messages as f64 / self.total_batches as f64
        }
    }
}

impl std::fmt::Display for IndexingTimingStats {
    #[allow(clippy::cast_precision_loss)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total = self.total_time();
        let prep_pct = if total.as_micros() > 0 {
            self.prep_time.as_micros() as f64 / total.as_micros() as f64 * 100.0
        } else {
            0.0
        };
        let index_pct = if total.as_micros() > 0 {
            self.index_time.as_micros() as f64 / total.as_micros() as f64 * 100.0
        } else {
            0.0
        };
        let cleanup_pct = if total.as_micros() > 0 {
            self.cleanup_time.as_micros() as f64 / total.as_micros() as f64 * 100.0
        } else {
            0.0
        };

        writeln!(f, "Indexing Timing Statistics:")?;
        writeln!(
            f,
            "  Total indexed: {} messages in {} batches (avg {:.1} msgs/batch)",
            self.total_messages,
            self.total_batches,
            self.avg_batch_size()
        )?;
        writeln!(f)?;
        writeln!(f, "  Aggregate times (single worker):")?;
        writeln!(
            f,
            "    Preparation (DB read, HTML→text): {:>8.2}s ({:>5.1}%)",
            self.prep_time.as_secs_f64(),
            prep_pct
        )?;
        writeln!(
            f,
            "    Foundation Search indexing:       {:>8.2}s ({:>5.1}%)",
            self.index_time.as_secs_f64(),
            index_pct
        )?;
        writeln!(
            f,
            "    Cleanup (save hash, delete):      {:>8.2}s ({:>5.1}%)",
            self.cleanup_time.as_secs_f64(),
            cleanup_pct
        )?;
        writeln!(
            f,
            "    Total measured:                   {:>8.2}s",
            total.as_secs_f64()
        )?;
        writeln!(f)?;
        writeln!(f, "  Average per message:")?;
        writeln!(
            f,
            "    Preparation:  {:>6.2}ms",
            self.avg_prep_per_message().as_secs_f64() * 1000.0
        )?;
        writeln!(
            f,
            "    Indexing:     {:>6.2}ms",
            self.avg_index_per_message().as_secs_f64() * 1000.0
        )?;
        writeln!(
            f,
            "    Cleanup:      {:>6.2}ms",
            self.avg_cleanup_per_message().as_secs_f64() * 1000.0
        )?;
        Ok(())
    }
}
