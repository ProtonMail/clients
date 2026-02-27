use crate::datatypes::LocalMessageId;
use mail_core_common::datatypes::UnixTimestamp;
use mail_stash::{UserDb, macros::Model};

#[derive(Clone, Debug, Eq, PartialEq, Model)]
#[TableName("message_trackers")]
#[Database(UserDb)]
pub struct MessageTracker {
    #[IdField]
    pub local_message_id: LocalMessageId,

    #[DbField]
    pub last_checked_at: UnixTimestamp,
}
