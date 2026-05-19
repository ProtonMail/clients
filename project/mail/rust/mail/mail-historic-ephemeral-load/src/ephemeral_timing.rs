//! Per-run timing for ephemeral historic-load indexing.
//!
//! Tracks stage timings for perf analysis:
//! - metadata save stage
//! - decrypt stage
//! - HTML strip stage
//! - indexing stage (Foundation Search commit only)

use std::time::Duration;

/// Accumulates stage timings for a single [`super::ephemeral::ephemeral_index_only_messages`] run.
#[derive(Debug, Clone, Default)]
pub struct EphemeralTimingCollector {
    metadata_save_time: Duration,
    decrypt_time: Duration,
    html_strip_time: Duration,
    index_only_time: Duration,
    metadata_save_count: u64,
    decrypt_count: u64,
    html_strip_count: u64,
    index_count: u64,
}

impl EphemeralTimingCollector {
    pub fn record_metadata_save(&mut self, duration: Duration, message_count: usize) {
        self.metadata_save_time += duration;
        self.metadata_save_count += message_count as u64;
    }

    pub fn record_decrypt(&mut self, duration: Duration) {
        self.decrypt_time += duration;
        self.decrypt_count += 1;
    }

    pub fn record_html_strip(&mut self, duration: Duration) {
        self.html_strip_time += duration;
        self.html_strip_count += 1;
    }

    pub fn record_index_only(&mut self, duration: Duration, message_count: usize) {
        self.index_only_time += duration;
        self.index_count += message_count as u64;
    }

    pub fn snapshot(&self) -> EphemeralTimingStats {
        EphemeralTimingStats {
            metadata_save_time: self.metadata_save_time,
            metadata_save_count: self.metadata_save_count,
            decrypt_time: self.decrypt_time,
            html_strip_time: self.html_strip_time,
            index_only_time: self.index_only_time,
            decrypt_count: self.decrypt_count,
            html_strip_count: self.html_strip_count,
            index_count: self.index_count,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EphemeralTimingStats {
    pub metadata_save_time: Duration,
    pub metadata_save_count: u64,
    pub decrypt_time: Duration,
    pub html_strip_time: Duration,
    pub index_only_time: Duration,
    pub decrypt_count: u64,
    pub html_strip_count: u64,
    pub index_count: u64,
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
            "    Metadata save stage:       {:>8.2}s ({} messages)",
            self.metadata_save_time.as_secs_f64(),
            self.metadata_save_count
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
