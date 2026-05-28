//! Engine commit output held until SQLite persistence (historic load ACID page).

use crate::engine::IndexResult;

/// Blob writes from a Foundation Search `commit()`, not yet stored in SQLite.
#[derive(Debug, Clone)]
pub struct PreparedIndexCommit {
    pub save_operations: Vec<(String, Vec<u8>)>,
    pub cleanup_needed: bool,
}

impl PreparedIndexCommit {
    #[must_use]
    pub fn from_save_operations(save_operations: Vec<(String, Vec<u8>)>) -> Self {
        Self {
            save_operations,
            cleanup_needed: true,
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self {
            save_operations: Vec::new(),
            cleanup_needed: false,
        }
    }

    #[must_use]
    pub fn index_result(&self) -> IndexResult {
        if self.cleanup_needed {
            IndexResult::needs_cleanup()
        } else {
            IndexResult::no_cleanup()
        }
    }
}
