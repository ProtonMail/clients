use crate::models::LabelWithCounters;
use mail_core_common::datatypes::{LocalLabelId, SystemLabel};
use mail_stash::orm::Model;

use super::ViewMode;

#[derive(Debug, Clone, PartialEq)]
pub struct CategoryLabel {
    pub local_id: LocalLabelId,
    pub system_label: SystemLabel,
    pub unread: u64,
    pub has_unseen_items: bool,
    pub enabled: bool,
}

impl CategoryLabel {
    pub fn new(
        system_label: SystemLabel,
        lwc: &LabelWithCounters,
        view_mode: ViewMode,
        extra_unread: u64,
        enabled: bool,
    ) -> Self {
        let local_id = lwc.id();
        let label_unread = if view_mode == ViewMode::Conversations {
            lwc.unread_conv
        } else {
            lwc.unread_msg
        };
        let unread = label_unread + extra_unread;
        Self {
            local_id,
            system_label,
            unread,
            has_unseen_items: lwc.last_unseen_message.is_some(),
            enabled,
        }
    }
}
