use crate::datatypes::LocalAttachmentId;
use crate::models::{
    DraftAttachmentMetadata, DraftAttachmentUploadState, DraftSendResult, MetadataId,
};
use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashError, Tether, WatcherHandle};
use std::collections::HashSet;

#[cfg(test)]
#[path = "../tests/draft/observer.rs"]
mod tests;

/// A watcher for new draft send results.
///
/// Unlike other watchers, this watcher only returns new entries that are added to the table
/// after its creation.
pub struct DraftSendResultWatcher {
    watcher_handle: WatcherHandle,
    mail_stash: Stash<UserDb>,
    unseen: HashSet<DraftSendResult>,
    mode: DraftSendResultWatcherMode,
}

#[derive(Debug, Copy, Clone)]
pub enum DraftSendResultWatcherMode {
    /// Receive all unseen notifications
    All,
    /// Receive only notifications
    SentOnly,
}

impl DraftSendResultWatcher {
    /// Create a new instance with the given `mail_stash` db pool.
    pub async fn new(
        mail_stash: Stash<UserDb>,
        mode: DraftSendResultWatcherMode,
    ) -> Result<Self, StashError> {
        let conn = mail_stash.connection().await?;

        let all_unseen = Self::load_send_results(mode, &conn).await?;

        let handle = DraftSendResult::watch(&mail_stash).await?;

        Ok(Self {
            watcher_handle: handle,
            mail_stash,
            unseen: HashSet::from_iter(all_unseen),
            mode,
        })
    }

    /// Wait on the next new send result.
    pub async fn next(&mut self) -> Result<Vec<DraftSendResult>, StashError> {
        loop {
            self.watcher_handle
                .receiver
                .recv_async()
                .await
                .map_err(|_| StashError::WatcherError("Connection Lost".to_owned()))?;

            let mut all_unseen =
                Self::load_send_results(self.mode, &self.mail_stash.connection().await?).await?;

            if all_unseen.is_empty() {
                continue;
            }

            let new_state = HashSet::from_iter(all_unseen.clone());
            if new_state == self.unseen {
                continue;
            }

            all_unseen.retain(|v| !self.unseen.contains(v));

            self.unseen = new_state;

            if all_unseen.is_empty() {
                continue;
            }

            return Ok(all_unseen);
        }
    }

    async fn load_send_results(
        mode: DraftSendResultWatcherMode,
        tether: &Tether,
    ) -> Result<Vec<DraftSendResult>, StashError> {
        match mode {
            DraftSendResultWatcherMode::All => DraftSendResult::unseen(tether).await,
            DraftSendResultWatcherMode::SentOnly => {
                DraftSendResult::unseen_with_send_action(tether).await
            }
        }
    }
}

/// Observe attachment state for a given draft.
pub struct DraftAttachmentObserver {
    id: MetadataId,
    mail_stash: Stash<UserDb>,
    current: HashSet<DraftAttachmentMetadataObserverState>,
    watcher_handle: WatcherHandle,
}

impl DraftAttachmentObserver {
    /// Create new instance for the given `metadata_id`.
    pub async fn new(
        metadata_id: MetadataId,
        mail_stash: Stash<UserDb>,
    ) -> Result<Self, StashError> {
        let conn = mail_stash.connection().await?;

        let current = DraftAttachmentMetadata::find_by_metadata_id(metadata_id, &conn).await?;

        let handle = DraftAttachmentMetadata::watch(&mail_stash).await?;

        Ok(Self {
            id: metadata_id,
            mail_stash,
            current: HashSet::from_iter(
                current
                    .into_iter()
                    .filter(|v| !v.deleted)
                    .map(DraftAttachmentMetadataObserverState::from),
            ),
            watcher_handle: handle,
        })
    }

    pub async fn next(&mut self) -> Result<(), StashError> {
        loop {
            self.watcher_handle
                .receiver
                .recv_async()
                .await
                .map_err(|_| StashError::WatcherError("Connection Lost".to_owned()))?;

            let conn = self.mail_stash.connection().await?;
            let current = DraftAttachmentMetadata::find_by_metadata_id(self.id, &conn).await?;
            let new_state_set = HashSet::from_iter(
                current
                    .into_iter()
                    .filter(|v| !v.deleted)
                    .map(DraftAttachmentMetadataObserverState::from),
            );

            // No changes continue;
            if new_state_set == self.current {
                continue;
            }

            self.current = new_state_set;
            return Ok(());
        }
    }
}

/// Custom type to track changes, not all table changes need to be reported.
#[derive(Debug, Eq, PartialEq, Hash)]
struct DraftAttachmentMetadataObserverState {
    attachment_id: LocalAttachmentId,
    state: DraftAttachmentUploadState,
}

impl From<DraftAttachmentMetadata> for DraftAttachmentMetadataObserverState {
    fn from(metadata: DraftAttachmentMetadata) -> Self {
        Self {
            attachment_id: metadata.local_attachment_id,
            state: metadata.state(),
        }
    }
}
