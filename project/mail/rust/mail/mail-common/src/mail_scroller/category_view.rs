use std::collections::BTreeSet;

use crate::MailContextError;
use crate::datatypes::ViewMode;
use crate::models::{LabelWithCounters, MailSettings};
use mail_api_labels::LabelId;
use mail_core_common::datatypes::{LocalLabelId, SystemLabel};
use mail_core_common::models::Label;
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::{Stash, StashError, Tether, WatcherHandle};
use sqlite_watcher::watcher::TableObserver;
use tracing::error;

pub use crate::datatypes::CategoryLabel;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CategoryView {
    pub enabled: Option<LocalLabelId>,
    pub available: Vec<LocalLabelId>,
    pub filter_ids: Vec<LocalLabelId>,
}

impl CategoryView {
    /// Returns `CategoryView::default()` (empty) when:
    /// - `label` is not the Inbox (only Inbox supports category filtering), or
    /// - `mail_category_view = false` in `MailSettings`.
    pub async fn load(label: LocalLabelId, tether: &Tether) -> anyhow::Result<Self> {
        // Category filtering is only supported for the Inbox label.
        let inbox_local_id = SystemLabel::Inbox.local_id(tether).await?;
        if inbox_local_id != Some(label) {
            return Ok(Self::default());
        }

        let mail_category_view = MailSettings::get(tether)
            .await?
            .is_some_and(|s| s.mail_category_view);

        if !mail_category_view {
            return Ok(Self::default());
        }

        // If the mail_category_view setting is enabled, populate available categories
        // and auto-enable CategoryDefault (server-authoritative, bypasses enable() guard).
        let remote_ids = SystemLabel::category_labels().map(|sl| sl.remote_id());
        let labels = LabelWithCounters::from_remote_ids(tether, remote_ids).await?;
        let available = labels
            .iter()
            .filter(|lwc| {
                // CategoryDefault is the bin for display=0 labels — always include it.
                // For all other categories, only include if display=1.
                let system_label = SystemLabel::from_opt_rid(lwc.label.remote_id.as_ref());
                system_label == Some(SystemLabel::CategoryDefault) || lwc.label.display
            })
            .filter_map(|lwc| lwc.label.local_id)
            .collect();

        let enabled = SystemLabel::CategoryDefault.local_id(tether).await?;
        let filter_ids = Self::resolve_filter_ids(enabled, &labels);

        Ok(Self {
            enabled,
            available,
            filter_ids,
        })
    }

    pub async fn enable(
        &mut self,
        enable: Option<LocalLabelId>,
        tether: &Tether,
    ) -> Result<&Self, MailContextError> {
        if let Some(cat) = enable
            && !self.available.contains(&cat)
        {
            return Err(MailContextError::CategoryNotSupported);
        }
        // Do not allow to disable category with Some(enabled) field
        let enable = enable.or(self.enabled);

        self.enabled = enable;

        if enable.is_none() {
            self.filter_ids = vec![];
            return Ok(self);
        }

        let remote_ids: Vec<LabelId> = SystemLabel::category_labels()
            .into_iter()
            .map(|l| l.remote_id())
            .collect();
        let labels = LabelWithCounters::from_remote_ids(tether, remote_ids)
            .await
            .map_err(MailContextError::Other)?;
        self.filter_ids = Self::resolve_filter_ids(enable, &labels);

        Ok(self)
    }

    /// Returns fully-resolved category labels, including live unread counts and
    /// the enabled flag. The FFI layer calls this and converts via `From::from`.
    ///
    /// **Load scope**: always fetches all [`SystemLabel::category_labels()`] from the DB,
    /// regardless of `self.available`. Labels with `display=false` are excluded from the
    /// returned `Vec` but their unread counts are folded into
    /// `CategoryDefault.has_unseen_items` — because disabled categories are binned under
    /// `CategoryDefault` from the user's perspective.
    ///
    /// **Output scope**: only labels whose local ID is in `self.available` are returned.
    /// Do not widen `self.available` to include display=false labels — doing so would
    /// break the [`Self::enable`] guard and allow disabled categories to be selected.
    pub async fn into_labels(&self, tether: &Tether) -> anyhow::Result<Vec<CategoryLabel>> {
        let enabled = self.enabled;

        // Load ALL known category labels — do NOT scope this to self.available.
        let remote_ids = SystemLabel::category_labels().map(|sl| sl.remote_id());
        let all_lwcs = LabelWithCounters::from_remote_ids(tether, remote_ids).await?;

        let view_mode = MailSettings::get_or_default(tether).await.view_mode;

        // Aggregate unseen from labels not in self.available (i.e. display=false).
        // These carry over to CategoryDefault.
        let unavailable_unread: u64 = all_lwcs
            .iter()
            .filter(|lwc| {
                !lwc.label
                    .local_id
                    .is_some_and(|id| self.available.contains(&id))
            })
            .map(|lwc| {
                if view_mode == ViewMode::Conversations {
                    lwc.unread_conv
                } else {
                    lwc.unread_msg
                }
            })
            .sum();

        Ok(all_lwcs
            .into_iter()
            .filter(|lwc| {
                lwc.label
                    .local_id
                    .is_some_and(|id| self.available.contains(&id))
            })
            .filter_map(|lwc| {
                let system_label = SystemLabel::from_rid(lwc.label.remote_id.as_ref()?)?;
                let is_default = system_label == SystemLabel::CategoryDefault;
                Some(CategoryLabel::new(
                    system_label,
                    &lwc,
                    view_mode,
                    if is_default { unavailable_unread } else { 0 },
                    enabled == Some(lwc.id()),
                ))
            })
            .collect())
    }

    pub async fn watch(mail_stash: &Stash<UserDb>) -> Result<WatcherHandle, StashError> {
        let tables = Self::watched_tables();

        mail_stash
            .subscribe_to(move |sender| Box::new(CategoryViewWatcher { sender, tables }))
            .await
    }

    fn watched_tables() -> Vec<String> {
        vec![
            MailSettings::table_name().to_owned(),
            Label::table_name().to_owned(),
        ]
    }

    fn resolve_filter_ids(
        enabled: Option<LocalLabelId>,
        labels: &[LabelWithCounters],
    ) -> Vec<LocalLabelId> {
        let Some(enabled_id) = enabled else {
            return vec![];
        };

        let enabled_is_default = labels.iter().any(|lwc| {
            lwc.label.local_id == Some(enabled_id)
                && SystemLabel::from_opt_rid(lwc.label.remote_id.as_ref())
                    == Some(SystemLabel::CategoryDefault)
        });

        if !enabled_is_default {
            return vec![enabled_id];
        }

        labels
            .iter()
            .filter(|lwc| {
                let system_label = SystemLabel::from_opt_rid(lwc.label.remote_id.as_ref());
                system_label == Some(SystemLabel::CategoryDefault) || !lwc.label.display
            })
            .filter_map(|lwc| lwc.label.local_id)
            .collect()
    }
}

struct CategoryViewWatcher {
    sender: flume::Sender<()>,
    tables: Vec<String>,
}

impl TableObserver for CategoryViewWatcher {
    fn tables(&self) -> Vec<String> {
        self.tables.clone()
    }

    fn on_tables_changed(&self, _changed_tables: &BTreeSet<String>) {
        self.sender
            .send(())
            .inspect_err(|e| {
                error!(
                    "Failed to send notification for MailScrollerWatcher: {:?}",
                    e
                );
            })
            .ok();
    }
}
