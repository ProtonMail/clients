//! Search result types with highlighting metadata

/// A search result with highlighting metadata
#[derive(Clone, Debug, uniffi::Record)]
pub struct SearchResultWithHighlighting {
    /// The local message ID as a string
    pub message_id: String,
    /// Relevance score (0.0 to 1.0, higher is better)
    /// Currently not used, set to 0.0 when created from SearchScroller
    pub score: f64,
    /// Match occurrences for highlighting
    pub matches: Vec<SearchMatchPosition>,
}

/// A single match occurrence within a search result
#[derive(Clone, Debug, uniffi::Record)]
pub struct SearchMatchPosition {
    /// The attribute that matched: "subject", "body", "from_name", "from_email", "to", "cc", "bcc"
    pub attribute: String,
    /// Character position within the attribute value (0-based)
    pub position: u64,
    /// Value index (for multi-valued attributes like "to", "cc")
    pub value_index: u64,
}
