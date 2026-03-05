use crate::AppError;
use crate::datatypes::LabelDescription;
use crate::datatypes::labels::messages_counts;
use mail_core_common::datatypes::{LocalLabelId, SystemLabel as Sl};
use mail_core_common::models::Label;
use mail_stash::orm::Model;
use mail_stash::stash::Tether;

pub struct SystemLabel {
    pub local_id: LocalLabelId,
    pub description: LabelDescription,
    pub display: bool,
    pub name: String,
    pub notify: bool,
    pub display_order: u32,
    pub sticky: bool,
    pub count: u64,
}

impl SystemLabel {
    pub async fn new(label: &Label, tether: &Tether) -> Result<Self, AppError> {
        let (unread, total) = messages_counts(label, tether).await?;
        let label_description = LabelDescription::new(label);
        let count = match Sl::from_opt_rid(label.remote_id.as_ref()) {
            Some(Sl::Snoozed | Sl::Scheduled) => total,
            _ => unread,
        };

        Ok(Self {
            local_id: label.id(),
            display: label.display,
            description: label_description,
            name: label.name.clone(),
            notify: label.notify,
            display_order: label.display_order,
            sticky: label.sticky,
            count,
        })
    }

    pub async fn from_labels(labels: &[Label], tether: &Tether) -> Result<Vec<Self>, AppError> {
        let mut result = Vec::with_capacity(labels.len());
        for label in labels {
            let label = Self::new(label, tether).await?;
            result.push(label);
        }
        Ok(result)
    }
}
