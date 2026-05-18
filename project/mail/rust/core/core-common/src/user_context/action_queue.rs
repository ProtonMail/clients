use crate::models::LabelError;
use mail_action_queue::action::{self, WriterGuardError};
use mail_action_queue::queue::ActionRequeueReason;
use mail_core_api::service::ApiServiceError;
use mail_stash::stash::StashError;

#[derive(Debug, thiserror::Error)]
pub enum CoreActionError {
    #[error("Http: {0}")]
    Http(#[from] ApiServiceError),
    #[error("Stash: {0}")]
    Stash(#[from] StashError),
    #[error("Label: {0}")]
    Label(#[from] LabelError),
    #[error("No input provided")]
    NoInput,
    #[error("Queue Writer Guard Expired")]
    QueueWriterGuardExpired,
    #[error("Other: {0}")]
    Other(anyhow::Error),
}

impl action::Error for CoreActionError {
    fn can_requeue(&self) -> Option<ActionRequeueReason> {
        match self {
            Self::Http(e) if e.is_network_failure() => Some(ActionRequeueReason::NetworkFailed),
            Self::QueueWriterGuardExpired => Some(ActionRequeueReason::GuardExpired),
            _ => None,
        }
    }
}

impl From<WriterGuardError> for CoreActionError {
    fn from(value: WriterGuardError) -> Self {
        match value {
            WriterGuardError::Expired => Self::QueueWriterGuardExpired,
            WriterGuardError::Stash(e) => Self::Stash(e),
        }
    }
}
