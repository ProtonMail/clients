use crate::models::LabelWithCounters;
use mail_core_common::datatypes::{LocalLabelId, SystemLabel};
use mail_stash::orm::Model;

#[derive(Debug, Clone, PartialEq)]
pub struct CategoryLabel {
    pub local_id: LocalLabelId,
    pub system_label: SystemLabel,
    pub has_unseen_items: bool,
    pub enabled: bool,
}

impl From<(SystemLabel, LabelWithCounters)> for CategoryLabel {
    fn from((system_label, lwc): (SystemLabel, LabelWithCounters)) -> Self {
        Self {
            local_id: lwc.id(),
            system_label,
            has_unseen_items: lwc.unread_msg > 0 || lwc.unread_conv > 0,
            enabled: false,
        }
    }
}
