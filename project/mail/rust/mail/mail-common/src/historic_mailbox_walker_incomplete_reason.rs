//! Walker-native stop reasons not tied to a specific observer implementation.

use mail_stash::rusqlite::types::{
    FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef,
};

/// Why a historic walk stopped short of declaring full mailbox completion.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum HistoricMailboxWalkerIncompleteReason {
    /// Final batch had messages skipped (e.g. missing body); a re-run should
    /// retry the tail of the mailbox.
    SkippedBodiesAtTail,
}

impl HistoricMailboxWalkerIncompleteReason {
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::SkippedBodiesAtTail => "incomplete_with_skipped_bodies",
        }
    }

    #[must_use]
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "incomplete_with_skipped_bodies" => Some(Self::SkippedBodiesAtTail),
            _ => None,
        }
    }
}

impl std::fmt::Display for HistoricMailboxWalkerIncompleteReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

impl ToSql for HistoricMailboxWalkerIncompleteReason {
    fn to_sql(&self) -> mail_stash::rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(
            self.as_db_str().as_bytes(),
        )))
    }
}

impl FromSql for HistoricMailboxWalkerIncompleteReason {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Self::from_db_str(value.as_str()?).ok_or_else(|| {
            FromSqlError::Other(
                format!(
                    "unknown HistoricMailboxWalkerIncompleteReason: {}",
                    value.as_str().unwrap_or("")
                )
                .into(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_reason_round_trips() {
        let reason = HistoricMailboxWalkerIncompleteReason::SkippedBodiesAtTail;
        assert_eq!(
            HistoricMailboxWalkerIncompleteReason::from_db_str(reason.as_db_str()),
            Some(reason)
        );
    }
}
