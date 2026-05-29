use mail_common::datatypes::{
    MovableCategoryFolder as RealMovableCategoryFolder,
    MovableSystemFolder as RealMovableSystemFolder,
};
use uniffi::Enum as UniffiEnum;

/// System folders (non-category) that are valid move-to destinations.
/// Their values correspond to the remote ids of the labels in the core API database.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, UniffiEnum)]
#[repr(u8)]
pub enum MovableSystemFolder {
    Inbox = 0,
    Trash = 3,
    Spam = 4,
    Archive = 6,
}

impl From<RealMovableSystemFolder> for MovableSystemFolder {
    fn from(label: RealMovableSystemFolder) -> Self {
        match label {
            RealMovableSystemFolder::Inbox => Self::Inbox,
            RealMovableSystemFolder::Trash => Self::Trash,
            RealMovableSystemFolder::Spam => Self::Spam,
            RealMovableSystemFolder::Archive => Self::Archive,
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
        }
    }
}

/// Category system folders that are valid move-to destinations.
///
/// Variant names keep the `Category*` prefix even though the enum name already
/// disambiguates, to prevent telemetry / Sentry string drift from existing data.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, UniffiEnum)]
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

impl From<RealMovableCategoryFolder> for MovableCategoryFolder {
    fn from(label: RealMovableCategoryFolder) -> Self {
        match label {
            RealMovableCategoryFolder::CategorySocial => Self::CategorySocial,
            RealMovableCategoryFolder::CategoryPromotions => Self::CategoryPromotions,
            RealMovableCategoryFolder::CategoryUpdates => Self::CategoryUpdates,
            RealMovableCategoryFolder::CategoryDefault => Self::CategoryDefault,
            RealMovableCategoryFolder::CategoryNewsletter => Self::CategoryNewsletter,
            RealMovableCategoryFolder::CategoryTransactions => Self::CategoryTransactions,
        }
    }
}

impl From<MovableCategoryFolder> for RealMovableCategoryFolder {
    fn from(label: MovableCategoryFolder) -> Self {
        match label {
            MovableCategoryFolder::CategorySocial => Self::CategorySocial,
            MovableCategoryFolder::CategoryPromotions => Self::CategoryPromotions,
            MovableCategoryFolder::CategoryUpdates => Self::CategoryUpdates,
            MovableCategoryFolder::CategoryDefault => Self::CategoryDefault,
            MovableCategoryFolder::CategoryNewsletter => Self::CategoryNewsletter,
            MovableCategoryFolder::CategoryTransactions => Self::CategoryTransactions,
        }
    }
}
