use crate::datatypes::ViewMode;
use crate::models::{ConversationCounter, MailSettings, MessageCounter};
use crate::{CategoryView, MailContextResult, MailUserContext};
use mail_core_common::datatypes::{LocalLabelId, SystemLabel};
use mail_core_common::models::{Label, ModelExtension as _};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::{Stash, StashError, Tether, WatcherHandle};
use sqlite_watcher::watcher::{DropRemoveTableObserverHandle, TableObserver};
use std::collections::BTreeSet;
use tracing::error;

pub struct UnreadCountHandle {
    pub drop_handle: DropRemoveTableObserverHandle,
    pub receiver: flume::Receiver<u64>,
}

pub enum UnreadWatchScope {
    CategoryConversations,
    CategoryMessages,
    Conversations,
    Messages,
}

impl UnreadWatchScope {
    pub fn new(view_mode: ViewMode, category: Option<LocalLabelId>) -> Self {
        if category.is_some() {
            match view_mode {
                ViewMode::Conversations => Self::CategoryConversations,
                ViewMode::Messages => Self::CategoryMessages,
            }
        } else {
            match view_mode {
                ViewMode::Conversations => Self::Conversations,
                ViewMode::Messages => Self::Messages,
            }
        }
    }

    fn tables(&self) -> Vec<String> {
        match self {
            Self::CategoryMessages => vec![
                MessageCounter::table_name().to_string(),
                MailSettings::table_name().to_string(),
                Label::table_name().to_string(),
            ],
            Self::CategoryConversations => vec![
                ConversationCounter::table_name().to_string(),
                MailSettings::table_name().to_string(),
                Label::table_name().to_string(),
            ],
            Self::Conversations => vec![ConversationCounter::table_name().to_string()],
            Self::Messages => vec![MessageCounter::table_name().to_string()],
        }
    }
}

pub(super) struct UnreadCountWatcher {
    sender: flume::Sender<()>,
    scope: UnreadWatchScope,
}

impl TableObserver for UnreadCountWatcher {
    fn tables(&self) -> Vec<String> {
        self.scope.tables()
    }

    fn on_tables_changed(&self, _changed_tables: &BTreeSet<String>) {
        self.sender
            .send(())
            .inspect_err(|e| {
                error!(
                    "Failed to send notification for UnreadCountWatcher: {:?}",
                    e
                );
            })
            .ok();
    }
}

impl UnreadCountWatcher {
    pub async fn watch(
        scope: UnreadWatchScope,
        stash: &Stash<UserDb>,
    ) -> Result<WatcherHandle, StashError> {
        stash
            .subscribe_to(|sender| Box::new(Self { sender, scope }))
            .await
    }
}

pub async fn resolve_unread(
    label_id: LocalLabelId,
    view_mode: ViewMode,
    category: Option<LocalLabelId>,
    ctx: &MailUserContext,
) -> MailContextResult<u64> {
    let tether = ctx.user_stash().connection();

    let Some(category_id) = category else {
        return resolve_unread_from_view_mode(label_id, view_mode, &tether).await;
    };

    let category_view = CategoryView::load(label_id, ctx).await?;
    let labels = category_view.into_labels(&tether).await?;
    let get_count = |id| labels.iter().find(|l| l.local_id == id).map(|l| l.unread);

    // Try for category, fallback to primary and if that is not enough fallback to the label.
    let count = match get_count(category_id) {
        Some(count) => count,
        None => {
            let primary_local_id = SystemLabel::CategoryDefault
                .local_id(&tether)
                .await?
                .expect("Must exist");
            let Some(count) = get_count(primary_local_id) else {
                return resolve_unread_from_view_mode(label_id, view_mode, &tether).await;
            };

            count
        }
    };

    Ok(count)
}

async fn resolve_unread_from_view_mode(
    label_id: LocalLabelId,
    view_mode: ViewMode,
    tether: &Tether,
) -> MailContextResult<u64> {
    Ok(match view_mode {
        ViewMode::Conversations => ConversationCounter::find_by_id(label_id, tether)
            .await?
            .map(|c| c.unread)
            .unwrap_or_default(),
        ViewMode::Messages => MessageCounter::find_by_id(label_id, tether)
            .await?
            .map(|c| c.unread)
            .unwrap_or_default(),
    })
}

#[cfg(test)]
#[path = "../tests/mailbox/unread_count_watcher.rs"]
mod tests;
