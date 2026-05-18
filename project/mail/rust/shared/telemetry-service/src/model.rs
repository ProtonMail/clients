use mail_stash::UserDb;
use mail_stash::macros::Model;

#[derive(Debug, Clone, PartialEq, Model)]
#[TableName("telemetry_events")]
#[Database(UserDb)]
pub struct TelemetryEventRow {
    #[IdField]
    pub id: String,

    #[DbField]
    pub event_data: String,
}
