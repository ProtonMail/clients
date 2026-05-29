use crate::datatypes::LabelType;
use mail_core_api::services::proton::LabelId;
use mail_core_common::datatypes::SystemLabel;
use mail_core_common::models::Label;
use serde::{Deserialize, Serialize};

/// System folders that are valid move-to destinations (non-category).
#[derive(
    Copy,
    Clone,
    Debug,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize
)]
#[repr(u8)]
pub enum MovableSystemFolder {
    Inbox = 0,
    Trash = 3,
    Spam = 4,
    Archive = 6,
}

impl MovableSystemFolder {
    pub(crate) fn new(label: &Label) -> Option<Self> {
        match label.label_type {
            LabelType::Label | LabelType::Folder | LabelType::System => {
                Self::from_rid(label.remote_id.as_ref())
            }
        }
    }

    fn from_rid(label_id: Option<&LabelId>) -> Option<Self> {
        let remote_id: u8 = label_id?.parse().ok()?;

        match remote_id {
            x if x == Self::Inbox as u8 => Some(Self::Inbox),
            x if x == Self::Trash as u8 => Some(Self::Trash),
            x if x == Self::Spam as u8 => Some(Self::Spam),
            x if x == Self::Archive as u8 => Some(Self::Archive),
            _ => None,
        }
    }
}

impl From<MovableSystemFolder> for SystemLabel {
    fn from(value: MovableSystemFolder) -> Self {
        match value {
            MovableSystemFolder::Inbox => SystemLabel::Inbox,
            MovableSystemFolder::Trash => SystemLabel::Trash,
            MovableSystemFolder::Spam => SystemLabel::Spam,
            MovableSystemFolder::Archive => SystemLabel::Archive,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("SystemLabel {0:?} is not a MovableSystemFolder")]
pub struct NotMovableSystemFolder(pub SystemLabel);

impl TryFrom<SystemLabel> for MovableSystemFolder {
    type Error = NotMovableSystemFolder;

    fn try_from(value: SystemLabel) -> Result<Self, Self::Error> {
        match value {
            SystemLabel::Inbox => Ok(MovableSystemFolder::Inbox),
            SystemLabel::Trash => Ok(MovableSystemFolder::Trash),
            SystemLabel::Spam => Ok(MovableSystemFolder::Spam),
            SystemLabel::Archive => Ok(MovableSystemFolder::Archive),
            other => Err(NotMovableSystemFolder(other)),
        }
    }
}

/// Category system folders that are valid move-to destinations.
///
/// Variant names intentionally keep the `Category*` prefix even though the enum name
/// already disambiguates, to prevent telemetry / Sentry string drift from existing data.
#[derive(
    Copy,
    Clone,
    Debug,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize
)]
#[repr(u8)]
#[allow(clippy::enum_variant_names)]
pub enum MovableCategoryFolder {
    CategorySocial = 20,
    CategoryPromotions = 21,
    CategoryUpdates = 22,
    CategoryDefault = 24,
    CategoryNewsletter = 25,
    CategoryTransactions = 26,
}

impl From<MovableCategoryFolder> for SystemLabel {
    fn from(value: MovableCategoryFolder) -> Self {
        match value {
            MovableCategoryFolder::CategorySocial => SystemLabel::CategorySocial,
            MovableCategoryFolder::CategoryPromotions => SystemLabel::CategoryPromotions,
            MovableCategoryFolder::CategoryUpdates => SystemLabel::CategoryUpdates,
            MovableCategoryFolder::CategoryDefault => SystemLabel::CategoryDefault,
            MovableCategoryFolder::CategoryNewsletter => SystemLabel::CategoryNewsletter,
            MovableCategoryFolder::CategoryTransactions => SystemLabel::CategoryTransactions,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("SystemLabel {0:?} is not a MovableCategoryFolder")]
pub struct NotMovableCategoryFolder(pub SystemLabel);

impl TryFrom<SystemLabel> for MovableCategoryFolder {
    type Error = NotMovableCategoryFolder;

    fn try_from(value: SystemLabel) -> Result<Self, Self::Error> {
        match value {
            SystemLabel::CategorySocial => Ok(MovableCategoryFolder::CategorySocial),
            SystemLabel::CategoryPromotions => Ok(MovableCategoryFolder::CategoryPromotions),
            SystemLabel::CategoryUpdates => Ok(MovableCategoryFolder::CategoryUpdates),
            SystemLabel::CategoryDefault => Ok(MovableCategoryFolder::CategoryDefault),
            SystemLabel::CategoryNewsletter => Ok(MovableCategoryFolder::CategoryNewsletter),
            SystemLabel::CategoryTransactions => Ok(MovableCategoryFolder::CategoryTransactions),
            other => Err(NotMovableCategoryFolder(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movable_system_folder_round_trip() {
        let pairs: &[(MovableSystemFolder, SystemLabel)] = &[
            (MovableSystemFolder::Inbox, SystemLabel::Inbox),
            (MovableSystemFolder::Trash, SystemLabel::Trash),
            (MovableSystemFolder::Spam, SystemLabel::Spam),
            (MovableSystemFolder::Archive, SystemLabel::Archive),
        ];

        for (folder, label) in pairs {
            let round_tripped = SystemLabel::from(*folder);
            assert_eq!(round_tripped, *label);
            let back = MovableSystemFolder::try_from(round_tripped).unwrap();
            assert_eq!(back, *folder);
        }
    }

    #[test]
    fn movable_category_folder_round_trip() {
        let pairs: &[(MovableCategoryFolder, SystemLabel)] = &[
            (
                MovableCategoryFolder::CategorySocial,
                SystemLabel::CategorySocial,
            ),
            (
                MovableCategoryFolder::CategoryPromotions,
                SystemLabel::CategoryPromotions,
            ),
            (
                MovableCategoryFolder::CategoryUpdates,
                SystemLabel::CategoryUpdates,
            ),
            (
                MovableCategoryFolder::CategoryDefault,
                SystemLabel::CategoryDefault,
            ),
            (
                MovableCategoryFolder::CategoryNewsletter,
                SystemLabel::CategoryNewsletter,
            ),
            (
                MovableCategoryFolder::CategoryTransactions,
                SystemLabel::CategoryTransactions,
            ),
        ];

        for (folder, label) in pairs {
            let round_tripped = SystemLabel::from(*folder);
            assert_eq!(round_tripped, *label);
            let back = MovableCategoryFolder::try_from(round_tripped).unwrap();
            assert_eq!(back, *folder);
        }
    }

    #[test]
    fn non_movable_system_labels_are_rejected() {
        let non_movable = [
            SystemLabel::Sent,
            SystemLabel::Drafts,
            SystemLabel::AllMail,
            SystemLabel::AllSent,
            SystemLabel::Starred,
        ];
        for label in non_movable {
            assert!(MovableSystemFolder::try_from(label).is_err());
            assert!(MovableCategoryFolder::try_from(label).is_err());
        }
    }

    #[test]
    fn category_labels_rejected_by_movable_system_folder() {
        let categories = [
            SystemLabel::CategorySocial,
            SystemLabel::CategoryPromotions,
            SystemLabel::CategoryUpdates,
            SystemLabel::CategoryDefault,
            SystemLabel::CategoryNewsletter,
            SystemLabel::CategoryTransactions,
        ];
        for label in categories {
            assert!(MovableSystemFolder::try_from(label).is_err());
        }
    }

    #[test]
    fn system_folders_rejected_by_movable_category_folder() {
        let folders = [
            SystemLabel::Inbox,
            SystemLabel::Trash,
            SystemLabel::Spam,
            SystemLabel::Archive,
        ];
        for label in folders {
            assert!(MovableCategoryFolder::try_from(label).is_err());
        }
    }
}
