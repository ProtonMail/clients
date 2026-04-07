//! Perf fixtures and search performance tooling for mail.
//!

#![deny(unsafe_code)]

pub mod fixture_bodies;
pub mod message_body_cache;
pub mod prefetch_timing;

pub use fixture_bodies::{FixtureError, try_substitute_perf_body};

/// MIME declared by the fixture source (JSONL `mime` field, batch API `mime`, etc.). Never inferred from body bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeclaredFixtureMime {
    #[default]
    TextHtml,
    TextPlain,
}

/// Raw substitute body text plus declared MIME for historic-load / perf indexing.
#[derive(Clone, Debug)]
pub struct SubstituteBody {
    pub body: String,
    pub mime: DeclaredFixtureMime,
}
