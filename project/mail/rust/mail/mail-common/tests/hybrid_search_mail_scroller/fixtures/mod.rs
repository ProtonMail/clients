//! Fixture data for hybrid search acceptance tests.

use serde::Deserialize;

/// Email fixture entry for hybrid search tests.
#[derive(Debug, Clone, Deserialize)]
pub struct HybridSearchEmailFixture {
    pub remote_id: String,
    pub subject: String,
    pub body: String,
}

/// Load the hybrid search email fixture (project, quarterly, budget).
pub fn hybrid_search_fixture() -> Vec<HybridSearchEmailFixture> {
    const JSON: &str = include_str!("hybrid_search_emails.json");
    serde_json::from_str(JSON).expect("hybrid_search_emails.json must be valid")
}
