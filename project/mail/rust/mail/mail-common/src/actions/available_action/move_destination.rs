use crate::datatypes::labels::hierarchy::Hierarchy;
use crate::datatypes::{MovableSystemFolder, SystemLabelId};
use crate::{AppError, CategoryLabel};
use mail_core_api::services::proton::LabelId;
use mail_core_common::datatypes::{LabelColor, LocalLabelId};
use mail_core_common::models::{Label, ModelIdExtension};
use mail_stash::stash::Tether;

#[derive(Debug, Clone, PartialEq)]
pub enum MoveDestination {
    /// Move to inbox
    Inbox(InboxDestination),

    /// Move to a system folder (e.g. Sent, Archive, Trash).
    SystemFolder(SystemFolderDestination),

    /// Move to a custom folder.
    CustomFolder(CustomFolderDestination),
}

/// This struct represents an Inbox with or without categories
///
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InboxDestination {
    pub local_id: LocalLabelId,
    pub name: MovableSystemFolder,
    pub categories: Vec<SystemFolderDestination>,
}

/// This struct represents a system folder that can be used as an action.
///
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct SystemFolderDestination {
    /// The database id of the label.
    pub local_id: LocalLabelId,

    /// The name of the system folder embedded as finite enum list.
    pub name: MovableSystemFolder,
}

impl SystemFolderDestination {
    pub(crate) fn from_label(label: &Label) -> Option<Self> {
        Some(Self {
            local_id: label.local_id?,
            name: MovableSystemFolder::new(label)?,
        })
    }

    pub(crate) fn from_categories(labels: Vec<CategoryLabel>) -> Vec<Self> {
        labels
            .into_iter()
            .filter_map(|label| {
                Some(Self {
                    local_id: label.local_id,
                    name: MovableSystemFolder::try_from(label.system_label).ok()?,
                })
            })
            .collect()
    }

    pub(crate) async fn inbox(tether: &Tether) -> Result<Self, AppError> {
        let local_id = Label::remote_id_counterpart(LabelId::inbox(), tether)
            .await?
            .expect("Should be set");

        Ok(Self {
            local_id,
            name: MovableSystemFolder::Inbox,
        })
    }

    pub(crate) async fn archive(tether: &Tether) -> Result<Self, AppError> {
        let local_id = Label::remote_id_counterpart(LabelId::archive(), tether)
            .await?
            .expect("Should be set");
        Ok(Self {
            local_id,
            name: MovableSystemFolder::Archive,
        })
    }

    pub(crate) async fn trash(tether: &Tether) -> Result<Self, AppError> {
        let local_id = Label::remote_id_counterpart(LabelId::trash(), tether)
            .await?
            .expect("Should be set");
        Ok(Self {
            local_id,
            name: MovableSystemFolder::Trash,
        })
    }

    pub(crate) async fn spam(tether: &Tether) -> Result<Self, AppError> {
        let local_id = Label::remote_id_counterpart(LabelId::spam(), tether)
            .await?
            .expect("Should be set");
        Ok(Self {
            local_id,
            name: MovableSystemFolder::Spam,
        })
    }
}

/// This struct represents a custom folder that can be used as an action.
///
#[derive(Debug, Clone, PartialEq)]
pub struct CustomFolderDestination {
    /// The database id of the label.
    pub local_id: LocalLabelId,

    /// The name of the folder.
    pub name: String,

    /// Folder color is calculated based on user settings.
    pub color: Option<LabelColor>,

    /// The order in which the folder should be displayed.
    pub display_order: u32,

    /// The parent folder of the current folder.
    pub parent: Option<LocalLabelId>,

    /// It holds folder structure as self reference within vector.
    pub children: Vec<CustomFolderDestination>,
}

impl CustomFolderDestination {
    pub(crate) fn from_label(label: &Label) -> Option<Self> {
        Some(Self {
            local_id: label.local_id?,
            name: label.name.clone(),
            color: None,
            parent: label.local_parent_id,
            display_order: label.display_order,
            children: vec![],
        })
    }
}

impl Default for CustomFolderDestination {
    fn default() -> Self {
        Self {
            local_id: LocalLabelId::from(0),
            name: String::default(),
            color: None,
            display_order: 0,
            parent: None,
            children: vec![],
        }
    }
}

impl Hierarchy for CustomFolderDestination {
    fn local_id(&self) -> u64 {
        self.local_id.as_u64()
    }

    fn parent_id(&self) -> Option<u64> {
        self.parent.map(|x| x.as_u64())
    }

    fn set_children(&mut self, children: Vec<Self>) {
        self.children = children;
    }

    fn display_order(&self) -> u32 {
        self.display_order
    }
}
