use mail_common::datatypes::MovableSystemFolder as RealMovableSystemFolder;
use uniffi::Enum as UniffiEnum;

/// This enum represents the system labels that are valid target for Move actions.
/// Their values correspond to the remote ids of the labels in the core API database.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, UniffiEnum)]
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

impl From<RealMovableSystemFolder> for MovableSystemFolder {
    fn from(label: RealMovableSystemFolder) -> Self {
        match label {
            RealMovableSystemFolder::Inbox => Self::Inbox,
            RealMovableSystemFolder::Trash => Self::Trash,
            RealMovableSystemFolder::Spam => Self::Spam,
            RealMovableSystemFolder::Archive => Self::Archive,
            RealMovableSystemFolder::CategorySocial => Self::CategorySocial,
            RealMovableSystemFolder::CategoryPromotions => Self::CategoryPromotions,
            RealMovableSystemFolder::CategoryUpdates => Self::CategoryUpdates,
            RealMovableSystemFolder::CategoryDefault => Self::CategoryDefault,
            RealMovableSystemFolder::CategoryNewsletter => Self::CategoryNewsletter,
            RealMovableSystemFolder::CategoryTransactions => Self::CategoryTransactions,
        }
    }
}

impl From<MovableSystemFolder> for RealMovableSystemFolder {
    fn from(label: MovableSystemFolder) -> Self {
        match label {
            MovableSystemFolder::Inbox => Self::Inbox,
            MovableSystemFolder::Trash => Self::Trash,
            MovableSystemFolder::Spam => Self::Spam,
            MovableSystemFolder::Archive => Self::Archive,
            MovableSystemFolder::CategorySocial => Self::CategorySocial,
            MovableSystemFolder::CategoryPromotions => Self::CategoryPromotions,
            MovableSystemFolder::CategoryUpdates => Self::CategoryUpdates,
            MovableSystemFolder::CategoryDefault => Self::CategoryDefault,
            MovableSystemFolder::CategoryNewsletter => Self::CategoryNewsletter,
            MovableSystemFolder::CategoryTransactions => Self::CategoryTransactions,
        }
    }
}
