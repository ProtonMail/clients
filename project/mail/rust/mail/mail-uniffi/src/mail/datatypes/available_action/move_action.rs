use mail_common::actions::{
    CategoryDestination as RealCategoryDestination,
    CustomFolderDestination as RealCustomFolderDestination,
    InboxDestination as RealInboxDestination, MoveDestination as RealMoveDestination,
    SystemFolderDestination as RealSystemFolderDestination,
};
use mail_core_common::utils::MapVec as _;

use crate::mail::datatypes::system_folder::{MovableCategoryFolder, MovableSystemFolder};
use crate::mail::datatypes::{Id, LabelColor};
use crate::{UniffiEnum, UniffiRecord};

/// This enum represents the action of moving a message or conversation to a folder.
///
#[derive(Debug, Clone, PartialEq, UniffiEnum)]
pub enum MoveDestination {
    /// Move to the inbox, optionally targeting one of its categories.
    Inbox(InboxDestination),

    /// Move to a system folder (e.g. Sent, Archive, Trash).
    SystemFolder(SystemFolderDestination),

    /// Move to a custom folder.
    CustomFolder(CustomFolderDestination),
}

impl From<RealMoveDestination> for MoveDestination {
    fn from(value: RealMoveDestination) -> Self {
        match value {
            RealMoveDestination::Inbox(value) => MoveDestination::Inbox(value.into()),
            RealMoveDestination::SystemFolder(value) => MoveDestination::SystemFolder(value.into()),
            RealMoveDestination::CustomFolder(value) => MoveDestination::CustomFolder(value.into()),
        }
    }
}

/// This struct represents the Inbox with its movable category sub-actions.
///
#[derive(Debug, Clone, PartialEq, UniffiRecord)]
pub struct InboxDestination {
    pub local_id: Id,
    pub name: MovableSystemFolder,
    pub categories: Vec<CategoryDestination>,
}

impl From<RealInboxDestination> for InboxDestination {
    fn from(value: RealInboxDestination) -> Self {
        Self {
            local_id: value.local_id.into(),
            name: value.name.into(),
            categories: value.categories.map_vec(),
        }
    }
}

/// This struct represents a system folder that can be used as an action.
///
#[derive(Debug, Clone, PartialEq, UniffiRecord)]
pub struct SystemFolderDestination {
    pub local_id: Id,
    pub name: MovableSystemFolder,
}

impl From<RealSystemFolderDestination> for SystemFolderDestination {
    fn from(value: RealSystemFolderDestination) -> Self {
        Self {
            local_id: value.local_id.into(),
            name: value.name.into(),
        }
    }
}

/// This struct represents a category folder that can be used as a move-to action.
///
#[derive(Debug, Clone, PartialEq, UniffiRecord)]
pub struct CategoryDestination {
    pub local_id: Id,
    pub name: MovableCategoryFolder,
}

impl From<RealCategoryDestination> for CategoryDestination {
    fn from(value: RealCategoryDestination) -> Self {
        Self {
            local_id: value.local_id.into(),
            name: value.name.into(),
        }
    }
}

/// This struct represents a custom folder that can be used as an action.
///
#[derive(Debug, Clone, PartialEq, UniffiRecord)]
pub struct CustomFolderDestination {
    pub local_id: Id,

    pub name: String,

    /// Folder color is calculated based on user settings.
    /// None means the folder colors are disabled.
    pub color: Option<LabelColor>,

    /// It holds folder structure as self reference within vector.
    pub children: Vec<CustomFolderDestination>,
}

impl From<RealCustomFolderDestination> for CustomFolderDestination {
    fn from(value: RealCustomFolderDestination) -> Self {
        CustomFolderDestination {
            local_id: value.local_id.into(),
            name: value.name.clone(),
            color: value.color.map(Into::into),
            children: value.children.map_vec(),
        }
    }
}
