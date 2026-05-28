//! Stable codes persisted in `content_search_indexing_state.last_error`.
//!
//! Mobile maps these to localized strings. Full diagnostics stay in Rust logs.

use mail_stash::rusqlite::types::{
    FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef,
};

/// Stable failure reason stored in the durable `last_error` column.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ContentSearchIndexingLastErrorCode {
    RetryableNetwork,
    FatalApi,
    CheckpointStorage,
    IndexingStateStorage,
    IndexPrepare,
    PagePersist,
    MetadataPrepare,
    InvalidContinuation,
    IncompleteWithSkippedBodies,
    StaleOngoingRecovered,
    Internal,
}

impl ContentSearchIndexingLastErrorCode {
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::RetryableNetwork => "retryable_network",
            Self::FatalApi => "fatal_api",
            Self::CheckpointStorage => "checkpoint_storage",
            Self::IndexingStateStorage => "indexing_state_storage",
            Self::IndexPrepare => "index_prepare",
            Self::PagePersist => "page_persist",
            Self::MetadataPrepare => "metadata_prepare",
            Self::InvalidContinuation => "invalid_continuation",
            Self::IncompleteWithSkippedBodies => "incomplete_with_skipped_bodies",
            Self::StaleOngoingRecovered => "stale_ongoing_recovered",
            Self::Internal => "internal",
        }
    }

    #[must_use]
    pub fn from_db_str(s: &str) -> Option<Self> {
        Some(match s {
            "retryable_network" => Self::RetryableNetwork,
            "fatal_api" => Self::FatalApi,
            "checkpoint_storage" => Self::CheckpointStorage,
            "indexing_state_storage" => Self::IndexingStateStorage,
            "index_prepare" => Self::IndexPrepare,
            "page_persist" => Self::PagePersist,
            "metadata_prepare" => Self::MetadataPrepare,
            "invalid_continuation" => Self::InvalidContinuation,
            "incomplete_with_skipped_bodies" => Self::IncompleteWithSkippedBodies,
            "stale_ongoing_recovered" => Self::StaleOngoingRecovered,
            "internal" => Self::Internal,
            _ => return None,
        })
    }
}

impl std::fmt::Display for ContentSearchIndexingLastErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

impl ToSql for ContentSearchIndexingLastErrorCode {
    fn to_sql(&self) -> mail_stash::rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(
            self.as_db_str().as_bytes(),
        )))
    }
}

impl FromSql for ContentSearchIndexingLastErrorCode {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let code = Self::from_db_str(value.as_str()?).ok_or_else(|| {
            FromSqlError::Other(
                format!(
                    "unknown ContentSearchIndexingLastErrorCode: {}",
                    value.as_str().unwrap_or("")
                )
                .into(),
            )
        })?;
        Ok(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_str_round_trips_for_every_variant() {
        for code in [
            ContentSearchIndexingLastErrorCode::RetryableNetwork,
            ContentSearchIndexingLastErrorCode::FatalApi,
            ContentSearchIndexingLastErrorCode::CheckpointStorage,
            ContentSearchIndexingLastErrorCode::IndexingStateStorage,
            ContentSearchIndexingLastErrorCode::IndexPrepare,
            ContentSearchIndexingLastErrorCode::PagePersist,
            ContentSearchIndexingLastErrorCode::MetadataPrepare,
            ContentSearchIndexingLastErrorCode::InvalidContinuation,
            ContentSearchIndexingLastErrorCode::IncompleteWithSkippedBodies,
            ContentSearchIndexingLastErrorCode::StaleOngoingRecovered,
            ContentSearchIndexingLastErrorCode::Internal,
        ] {
            let s = code.as_db_str();
            assert_eq!(
                ContentSearchIndexingLastErrorCode::from_db_str(s),
                Some(code),
                "round-trip failed for {s}"
            );
        }
    }

    #[test]
    fn unknown_db_str_returns_none() {
        assert!(ContentSearchIndexingLastErrorCode::from_db_str("network down").is_none());
    }
}
