use crate::datatypes::LabelType;
use mail_core_api::services::proton::LabelId;
use mail_core_common::datatypes::SystemLabel;
use mail_core_common::models::Label;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
pub enum MovableSystemFolder {
    Inbox = 0,
    Trash = 3,
    Spam = 4,
    Archive = 6,
    CategorySocial = 20,
    CategoryPromotions = 21,
    CategoryUpdates = 22,
    CategoryDefault = 24,
    CategoryNewsletter = 25,
    CategoryTransactions = 26,
}

impl MovableSystemFolder {
    pub(crate) fn new(label: &Label) -> Option<Self> {
        match label.label_type {
            LabelType::Label | LabelType::ContactGroup | LabelType::Folder | LabelType::System => {
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
            x if x == Self::CategorySocial as u8 => Some(Self::CategorySocial),
            x if x == Self::CategoryPromotions as u8 => Some(Self::CategoryPromotions),
            x if x == Self::CategoryUpdates as u8 => Some(Self::CategoryUpdates),
            x if x == Self::CategoryDefault as u8 => Some(Self::CategoryDefault),
            x if x == Self::CategoryNewsletter as u8 => Some(Self::CategoryNewsletter),
            x if x == Self::CategoryTransactions as u8 => Some(Self::CategoryTransactions),
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
            MovableSystemFolder::CategorySocial => SystemLabel::CategorySocial,
            MovableSystemFolder::CategoryPromotions => SystemLabel::CategoryPromotions,
            MovableSystemFolder::CategoryUpdates => SystemLabel::CategoryUpdates,
            MovableSystemFolder::CategoryDefault => SystemLabel::CategoryDefault,
            MovableSystemFolder::CategoryNewsletter => SystemLabel::CategoryNewsletter,
            MovableSystemFolder::CategoryTransactions => SystemLabel::CategoryTransactions,
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
            SystemLabel::CategorySocial => Ok(MovableSystemFolder::CategorySocial),
            SystemLabel::CategoryPromotions => Ok(MovableSystemFolder::CategoryPromotions),
            SystemLabel::CategoryUpdates => Ok(MovableSystemFolder::CategoryUpdates),
            SystemLabel::CategoryDefault => Ok(MovableSystemFolder::CategoryDefault),
            SystemLabel::CategoryNewsletter => Ok(MovableSystemFolder::CategoryNewsletter),
            SystemLabel::CategoryTransactions => Ok(MovableSystemFolder::CategoryTransactions),
            other => Err(NotMovableSystemFolder(other)),
        }
    }
}
