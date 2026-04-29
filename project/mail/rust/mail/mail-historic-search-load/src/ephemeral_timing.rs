//! Timing instrumentation for ephemeral historic-load indexing.
//!
//! Tracks only stage timings requested for perf analysis:
//! - decrypt stage
//! - HTML strip stage
//! - indexing stage (Foundation Search commit only)

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static DECRYPT_MICROS: AtomicU64 = AtomicU64::new(0);
static HTML_STRIP_MICROS: AtomicU64 = AtomicU64::new(0);
static INDEX_ONLY_MICROS: AtomicU64 = AtomicU64::new(0);

static DECRYPT_COUNT: AtomicU64 = AtomicU64::new(0);
static HTML_STRIP_COUNT: AtomicU64 = AtomicU64::new(0);
static INDEX_COUNT: AtomicU64 = AtomicU64::new(0);

fn add_micros(target: &AtomicU64, duration: Duration) {
    let micros = u64::try_from(duration.as_micros().min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
    target.fetch_add(micros, Ordering::Relaxed);
}

pub fn reset() {
    DECRYPT_MICROS.store(0, Ordering::Relaxed);
    HTML_STRIP_MICROS.store(0, Ordering::Relaxed);
    INDEX_ONLY_MICROS.store(0, Ordering::Relaxed);
    DECRYPT_COUNT.store(0, Ordering::Relaxed);
    HTML_STRIP_COUNT.store(0, Ordering::Relaxed);
    INDEX_COUNT.store(0, Ordering::Relaxed);
}

pub fn record_decrypt(duration: Duration) {
    add_micros(&DECRYPT_MICROS, duration);
    DECRYPT_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub fn record_html_strip(duration: Duration) {
    add_micros(&HTML_STRIP_MICROS, duration);
    HTML_STRIP_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub fn record_index_only(duration: Duration, message_count: usize) {
    add_micros(&INDEX_ONLY_MICROS, duration);
    INDEX_COUNT.fetch_add(message_count as u64, Ordering::Relaxed);
}

#[derive(Debug, Clone)]
pub struct EphemeralTimingStats {
    pub decrypt_time: Duration,
    pub html_strip_time: Duration,
    pub index_only_time: Duration,
    pub decrypt_count: u64,
    pub html_strip_count: u64,
    pub index_count: u64,
}

impl EphemeralTimingStats {
    pub fn snapshot() -> Self {
        Self {
            decrypt_time: Duration::from_micros(DECRYPT_MICROS.load(Ordering::Relaxed)),
            html_strip_time: Duration::from_micros(HTML_STRIP_MICROS.load(Ordering::Relaxed)),
            index_only_time: Duration::from_micros(INDEX_ONLY_MICROS.load(Ordering::Relaxed)),
            decrypt_count: DECRYPT_COUNT.load(Ordering::Relaxed),
            html_strip_count: HTML_STRIP_COUNT.load(Ordering::Relaxed),
            index_count: INDEX_COUNT.load(Ordering::Relaxed),
        }
    }
}

impl std::fmt::Display for EphemeralTimingStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Ephemeral Stage Timing Statistics:")?;
        writeln!(
            f,
            "  Totals (stage-only, excludes unrelated processor costs):"
        )?;
        writeln!(
            f,
            "    Decrypt stage:             {:>8.2}s ({} messages)",
            self.decrypt_time.as_secs_f64(),
            self.decrypt_count
        )?;
        writeln!(
            f,
            "    HTML strip stage:          {:>8.2}s ({} messages)",
            self.html_strip_time.as_secs_f64(),
            self.html_strip_count
        )?;
        writeln!(
            f,
            "    Foundation indexing only:  {:>8.2}s ({} messages)",
            self.index_only_time.as_secs_f64(),
            self.index_count
        )?;
        Ok(())
    }
}
