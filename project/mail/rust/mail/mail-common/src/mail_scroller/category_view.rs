use crate::MailContextError;
use crate::models::{LabelWithCounters, MailSettings};
use mail_api_labels::LabelId;
use mail_core_common::datatypes::{LocalLabelId, SystemLabel};
use mail_stash::orm::Model;
use mail_stash::stash::Tether;

pub use crate::datatypes::CategoryLabel;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CategoryView {
    pub enabled: Option<LocalLabelId>,
    pub available: Vec<LocalLabelId>,
}

impl CategoryView {
    pub async fn load(tether: &Tether) -> anyhow::Result<Self> {
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
            .into_iter()
            .filter(|lwc| {
                // CategoryDefault is the bin for display=0 labels — always include it.
                // For all other categories, only include if display=1.
                let system_label = SystemLabel::from_opt_rid(lwc.label.remote_id.as_ref());
                system_label == Some(SystemLabel::CategoryDefault) || lwc.label.display
            })
            .filter_map(|lwc| lwc.label.local_id)
            .collect();

        let enabled = SystemLabel::CategoryDefault.local_id(tether).await?;

        Ok(Self { enabled, available })
    }

    pub fn enable(&mut self, enable: Option<LocalLabelId>) -> Result<&Self, MailContextError> {
        if let Some(cat) = enable
            && !self.available.contains(&cat)
        {
            return Err(MailContextError::CategoryNotSupported);
        }
        self.enabled = enable;

        Ok(self)
    }

    /// Returns the SQL filter IDs for the active category selection.
    ///
    /// - `None` → empty vec (no category filter applied)
    /// - `CategoryDefault` → CategoryDefault's local ID + all category labels with `display=false`
    /// - Any other category → `vec![enabled_id]`
    pub async fn query_filter_ids(&self, tether: &Tether) -> anyhow::Result<Vec<LocalLabelId>> {
        let enabled_id = match self.enabled {
            None => return Ok(vec![]),
            Some(id) => id,
        };

        // Load all known category labels to classify the enabled ID and find display=0 labels.
        let remote_ids: Vec<LabelId> = SystemLabel::category_labels()
            .into_iter()
            .map(|l| l.remote_id())
            .collect();
        let labels = LabelWithCounters::from_remote_ids(tether, remote_ids).await?;

        // Determine if the enabled label is CategoryDefault.
        let enabled_is_default = labels.iter().any(|lwc| {
            lwc.label.local_id == Some(enabled_id)
                && SystemLabel::from_opt_rid(lwc.label.remote_id.as_ref())
                    == Some(SystemLabel::CategoryDefault)
        });

        if !enabled_is_default {
            return Ok(vec![enabled_id]);
        }

        // CategoryDefault: include its own ID + all display=0 category labels.
        let ids = labels
            .into_iter()
            .filter(|lwc| {
                let system_label = SystemLabel::from_opt_rid(lwc.label.remote_id.as_ref());
                system_label == Some(SystemLabel::CategoryDefault) || !lwc.label.display
            })
            .filter_map(|lwc| lwc.label.local_id)
            .collect();

        Ok(ids)
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

        // Aggregate unseen from labels not in self.available (i.e. display=false).
        // These carry over to CategoryDefault.has_unseen_items.
        let unavailable_has_unseen = all_lwcs.iter().any(|lwc| {
            !lwc.label
                .local_id
                .is_some_and(|id| self.available.contains(&id))
                && (lwc.unread_msg > 0 || lwc.unread_conv > 0)
        });

        Ok(all_lwcs
            .into_iter()
            .filter(|lwc| {
                lwc.label
                    .local_id
                    .is_some_and(|id| self.available.contains(&id))
            })
            .filter_map(|lwc| {
                let system_label = SystemLabel::from_rid(lwc.label.remote_id.as_ref()?)?;
                let local_id = lwc.label.id();
                let is_default = system_label == SystemLabel::CategoryDefault;
                Some(CategoryLabel {
                    local_id,
                    system_label,
                    has_unseen_items: lwc.unread_msg > 0
                        || lwc.unread_conv > 0
                        || (is_default && unavailable_has_unseen),
                    enabled: enabled == Some(local_id),
                })
            })
            .collect())
    }
}
