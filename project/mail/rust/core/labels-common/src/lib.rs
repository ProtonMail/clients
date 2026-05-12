pub mod db;
pub mod label;
pub mod label_type;
pub mod local_ids;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

pub use label::{Label, LabelError, LabelWatcher};
pub use label_type::{ALL_LABEL_TYPES, LabelColor, LabelType, Labels, MAIL_LABEL_TYPES};
pub use local_ids::LocalLabelId;
