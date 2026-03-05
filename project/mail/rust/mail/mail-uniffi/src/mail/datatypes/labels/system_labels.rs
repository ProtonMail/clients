use crate::core::datatypes::Id;
use crate::mail::datatypes::LabelDescription;
use mail_common::datatypes::labels::system_labels::SystemLabel as RealSystemLabel;
use uniffi::Record as UniffiRecord;

/// Representation of a `Label` defined by the system
#[derive(Clone, Debug, Eq, PartialEq, UniffiRecord)]
pub struct SidebarSystemLabel {
    /// Local id of the Label.
    pub id: Id,

    /// TODO: Document this field.
    pub display: bool,

    /// Description of this Label.
    pub description: LabelDescription,

    /// The name of this Label.
    pub name: String,

    /// TODO: Document this field.
    pub notify: bool,

    /// Order to display relative to other `CustomLabel`.
    pub display_order: u32,

    /// TODO: Document this field.
    pub sticky: bool,

    /// Count of the message to display in this Label.
    pub count: u64,
}

impl From<RealSystemLabel> for SidebarSystemLabel {
    fn from(value: RealSystemLabel) -> Self {
        Self {
            id: value.local_id.into(),
            description: value.description.into(),
            display: value.display,
            name: value.name,
            notify: value.notify,
            display_order: value.display_order,
            sticky: value.sticky,
            count: value.count,
        }
    }
}
