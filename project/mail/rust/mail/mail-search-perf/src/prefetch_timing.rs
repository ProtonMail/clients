//! Prefetch timing instrumentation for search perf analysis.
//

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Accumulated time for `sync_message_and_body()` (HTTP GET + metadata DB writes)
static HTTP_AND_METADATA_SAVE_MICROS: AtomicU64 = AtomicU64::new(0);

/// Accumulated time for `decrypt_message_body()` (decrypt + body DB save + index intent)
static DECRYPT_AND_BODY_SAVE_MICROS: AtomicU64 = AtomicU64::new(0);

static TOTAL_PREFETCH_COUNT: AtomicU64 = AtomicU64::new(0);
static CACHE_HIT_COUNT: AtomicU64 = AtomicU64::new(0);

fn add_micros(target: &AtomicU64, duration: Duration) {
    let micros = u64::try_from(duration.as_micros().min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
    target.fetch_add(micros, Ordering::Relaxed);
}

/// Stopwatch for a single message prefetch (HTTP phase then decrypt phase).
#[derive(Debug)]
pub struct PrefetchStopwatch {
    phase_start: Instant,
}

impl PrefetchStopwatch {
    /// Start timing before the HTTP / metadata phase.
    #[must_use]
    pub fn start() -> Self {
        Self {
            phase_start: Instant::now(),
        }
    }

    /// Record HTTP + metadata phase complete; begins the decrypt phase timer.
    #[must_use]
    pub fn record_http_done(self) -> Self {
        add_micros(&HTTP_AND_METADATA_SAVE_MICROS, self.phase_start.elapsed());
        Self {
            phase_start: Instant::now(),
        }
    }

    /// Record decrypt + body save complete and count one finished prefetch. Consumes `self`.
    pub fn record_decrypt_done(self) {
        add_micros(&DECRYPT_AND_BODY_SAVE_MICROS, self.phase_start.elapsed());
        TOTAL_PREFETCH_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache hit (no HTTP or decrypt timing for this load). Updates global counters only.
    pub fn record_cache_hit() {
        CACHE_HIT_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    /// Reset global timing counters (call at start of test / benchmark).
    pub fn reset_counters() {
        HTTP_AND_METADATA_SAVE_MICROS.store(0, Ordering::Relaxed);
        DECRYPT_AND_BODY_SAVE_MICROS.store(0, Ordering::Relaxed);
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
    pub http_and_metadata_save: Duration,
    pub decrypt_and_body_save: Duration,
    pub total_count: u64,
    pub cache_hits: u64,
}

impl PrefetchTimingStats {
    pub fn snapshot() -> Self {
        Self {
            http_and_metadata_save: Duration::from_micros(
                HTTP_AND_METADATA_SAVE_MICROS.load(Ordering::Relaxed),
            ),
            decrypt_and_body_save: Duration::from_micros(
                DECRYPT_AND_BODY_SAVE_MICROS.load(Ordering::Relaxed),
            ),
            total_count: TOTAL_PREFETCH_COUNT.load(Ordering::Relaxed),
            cache_hits: CACHE_HIT_COUNT.load(Ordering::Relaxed),
        }
    }

    #[must_use]
    pub fn total_measured_time(&self) -> Duration {
        self.http_and_metadata_save + self.decrypt_and_body_save
    }

    #[must_use]
    pub fn avg_http_time(&self) -> Duration {
        if self.total_count == 0 {
            Duration::ZERO
        } else {
            let divisor =
                u32::try_from(self.total_count.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            self.http_and_metadata_save / divisor
        }
    }

    #[must_use]
    pub fn avg_decrypt_time(&self) -> Duration {
        if self.total_count == 0 {
            Duration::ZERO
        } else {
            let divisor =
                u32::try_from(self.total_count.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            self.decrypt_and_body_save / divisor
        }
    }
}

impl std::fmt::Display for PrefetchTimingStats {
    #[allow(clippy::cast_precision_loss)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total = self.total_measured_time();
        let http_pct = if total.as_micros() > 0 {
            self.http_and_metadata_save.as_micros() as f64 / total.as_micros() as f64 * 100.0
        } else {
            0.0
        };
        let decrypt_pct = if total.as_micros() > 0 {
            self.decrypt_and_body_save.as_micros() as f64 / total.as_micros() as f64 * 100.0
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
            "    HTTP + metadata save:   {:>8.2}s ({:>5.1}%)",
            self.http_and_metadata_save.as_secs_f64(),
            http_pct
        )?;
        writeln!(
            f,
            "    Decrypt + body save:    {:>8.2}s ({:>5.1}%)",
            self.decrypt_and_body_save.as_secs_f64(),
            decrypt_pct
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
            "    HTTP + metadata:  {:>6.1}ms",
            self.avg_http_time().as_secs_f64() * 1000.0
        )?;
        writeln!(
            f,
            "    Decrypt + body:   {:>6.1}ms",
            self.avg_decrypt_time().as_secs_f64() * 1000.0
        )?;

        Ok(())
    }
}
