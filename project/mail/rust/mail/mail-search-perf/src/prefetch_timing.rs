//! Prefetch timing instrumentation for search perf analysis.
//

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// `api.get_message` call only.
static API_FETCH_MICROS: AtomicU64 = AtomicU64::new(0);
/// Message/body metadata conversion + metadata DB writes + rebase.
static METADATA_SAVE_MICROS: AtomicU64 = AtomicU64::new(0);
/// Decrypt + body processing.
static DECRYPT_ONLY_MICROS: AtomicU64 = AtomicU64::new(0);
/// Raw body save + index intent queueing + metadata save in decrypt transaction.
static BODY_STORE_AND_INDEX_INTENT_MICROS: AtomicU64 = AtomicU64::new(0);

static TOTAL_PREFETCH_COUNT: AtomicU64 = AtomicU64::new(0);
static CACHE_HIT_COUNT: AtomicU64 = AtomicU64::new(0);

fn add_micros(target: &AtomicU64, duration: Duration) {
    let micros = u64::try_from(duration.as_micros().min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
    target.fetch_add(micros, Ordering::Relaxed);
}

/// Stopwatch for a single message prefetch across phases.
#[derive(Debug)]
pub struct PrefetchStopwatch {
    phase_start: Instant,
}

impl PrefetchStopwatch {
    /// Start phase timing.
    #[must_use]
    pub fn start() -> Self {
        Self {
            phase_start: Instant::now(),
        }
    }

    /// Record API fetch complete; begins metadata phase timer.
    #[must_use]
    pub fn record_api_fetch_done(self) -> Self {
        add_micros(&API_FETCH_MICROS, self.phase_start.elapsed());
        Self {
            phase_start: Instant::now(),
        }
    }

    /// Record metadata phase complete.
    pub fn record_metadata_done(self) {
        add_micros(&METADATA_SAVE_MICROS, self.phase_start.elapsed());
    }

    /// Start decrypt phase explicitly after sync_message_and_body.
    #[must_use]
    pub fn start_decrypt_phase() -> Self {
        Self::start()
    }

    /// Record decrypt complete; begins store+queue phase timer.
    #[must_use]
    pub fn record_decrypt_done(self) -> Self {
        add_micros(&DECRYPT_ONLY_MICROS, self.phase_start.elapsed());
        Self {
            phase_start: Instant::now(),
        }
    }

    /// Record body store + queue complete and count one finished prefetch.
    pub fn record_store_and_index_done(self) {
        add_micros(
            &BODY_STORE_AND_INDEX_INTENT_MICROS,
            self.phase_start.elapsed(),
        );
        TOTAL_PREFETCH_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache hit (no HTTP or decrypt timing for this load). Updates global counters only.
    pub fn record_cache_hit() {
        CACHE_HIT_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    /// Reset global timing counters (call at start of test / benchmark).
    pub fn reset_counters() {
        API_FETCH_MICROS.store(0, Ordering::Relaxed);
        METADATA_SAVE_MICROS.store(0, Ordering::Relaxed);
        DECRYPT_ONLY_MICROS.store(0, Ordering::Relaxed);
        BODY_STORE_AND_INDEX_INTENT_MICROS.store(0, Ordering::Relaxed);
        TOTAL_PREFETCH_COUNT.store(0, Ordering::Relaxed);
        CACHE_HIT_COUNT.store(0, Ordering::Relaxed);
    }
}

impl Default for PrefetchStopwatch {
    fn default() -> Self {
        Self::start()
    }
}

/// Snapshot of prefetch timing statistics.
#[derive(Debug, Clone)]
pub struct PrefetchTimingStats {
    pub api_fetch: Duration,
    pub metadata_save: Duration,
    pub decrypt_only: Duration,
    pub body_store_and_index_intent: Duration,
    pub total_count: u64,
    pub cache_hits: u64,
}

impl PrefetchTimingStats {
    pub fn snapshot() -> Self {
        Self {
            api_fetch: Duration::from_micros(API_FETCH_MICROS.load(Ordering::Relaxed)),
            metadata_save: Duration::from_micros(METADATA_SAVE_MICROS.load(Ordering::Relaxed)),
            decrypt_only: Duration::from_micros(DECRYPT_ONLY_MICROS.load(Ordering::Relaxed)),
            body_store_and_index_intent: Duration::from_micros(
                BODY_STORE_AND_INDEX_INTENT_MICROS.load(Ordering::Relaxed),
            ),
            total_count: TOTAL_PREFETCH_COUNT.load(Ordering::Relaxed),
            cache_hits: CACHE_HIT_COUNT.load(Ordering::Relaxed),
        }
    }

    #[must_use]
    pub fn total_measured_time(&self) -> Duration {
        self.api_fetch + self.metadata_save + self.decrypt_only + self.body_store_and_index_intent
    }

    #[must_use]
    pub fn avg_api_fetch_time(&self) -> Duration {
        Self::avg_div(self.api_fetch, self.total_count)
    }

    #[must_use]
    pub fn avg_metadata_save_time(&self) -> Duration {
        Self::avg_div(self.metadata_save, self.total_count)
    }

    #[must_use]
    pub fn avg_decrypt_time(&self) -> Duration {
        Self::avg_div(self.decrypt_only, self.total_count)
    }

    #[must_use]
    pub fn avg_body_store_and_index_time(&self) -> Duration {
        Self::avg_div(self.body_store_and_index_intent, self.total_count)
    }

    fn avg_div(total: Duration, count: u64) -> Duration {
        if count == 0 {
            Duration::ZERO
        } else {
            let divisor = u32::try_from(count.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            total / divisor
        }
    }
}

impl std::fmt::Display for PrefetchTimingStats {
    #[allow(clippy::cast_precision_loss)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total = self.total_measured_time();
        let api_pct = if total.as_micros() > 0 {
            self.api_fetch.as_micros() as f64 / total.as_micros() as f64 * 100.0
        } else {
            0.0
        };
        let metadata_pct = if total.as_micros() > 0 {
            self.metadata_save.as_micros() as f64 / total.as_micros() as f64 * 100.0
        } else {
            0.0
        };
        let decrypt_pct = if total.as_micros() > 0 {
            self.decrypt_only.as_micros() as f64 / total.as_micros() as f64 * 100.0
        } else {
            0.0
        };
        let store_pct = if total.as_micros() > 0 {
            self.body_store_and_index_intent.as_micros() as f64 / total.as_micros() as f64 * 100.0
        } else {
            0.0
        };

        writeln!(f, "Prefetch Timing Statistics:")?;
        writeln!(
            f,
            "  Total operations: {} (cache hits: {})",
            self.total_count, self.cache_hits
        )?;
        writeln!(f)?;
        writeln!(f, "  Aggregate times (sum across all workers):")?;
        writeln!(
            f,
            "    API fetch:                {:>8.2}s ({:>5.1}%)",
            self.api_fetch.as_secs_f64(),
            api_pct
        )?;
        writeln!(
            f,
            "    Metadata save/rebase:     {:>8.2}s ({:>5.1}%)",
            self.metadata_save.as_secs_f64(),
            metadata_pct
        )?;
        writeln!(
            f,
            "    Decrypt only:             {:>8.2}s ({:>5.1}%)",
            self.decrypt_only.as_secs_f64(),
            decrypt_pct
        )?;
        writeln!(
            f,
            "    Body save + queue:        {:>8.2}s ({:>5.1}%)",
            self.body_store_and_index_intent.as_secs_f64(),
            store_pct
        )?;
        writeln!(
            f,
            "    Total measured:         {:>8.2}s",
            total.as_secs_f64()
        )?;
        writeln!(f)?;
        writeln!(f, "  Average per operation:")?;
        writeln!(
            f,
            "    API fetch:         {:>6.1}ms",
            self.avg_api_fetch_time().as_secs_f64() * 1000.0
        )?;
        writeln!(
            f,
            "    Metadata save:     {:>6.1}ms",
            self.avg_metadata_save_time().as_secs_f64() * 1000.0
        )?;
        writeln!(
            f,
            "    Decrypt:           {:>6.1}ms",
            self.avg_decrypt_time().as_secs_f64() * 1000.0
        )?;
        writeln!(
            f,
            "    Body save+queue:   {:>6.1}ms",
            self.avg_body_store_and_index_time().as_secs_f64() * 1000.0
        )?;

        Ok(())
    }
}
