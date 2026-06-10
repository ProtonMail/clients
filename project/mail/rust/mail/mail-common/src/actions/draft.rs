mod attachment_disposition_update;
mod attachment_remove;
mod attachment_upload;
mod discard;
mod save;
mod send;
mod undo_send;

use std::sync::OnceLock;

pub use self::attachment_disposition_update::*;
pub use self::attachment_remove::*;
pub use self::attachment_upload::*;
pub use self::discard::*;
pub use self::save::*;
pub use self::send::*;
pub use self::undo_send::*;
use crate::datatypes::{LocalAttachmentId, LocalMessageId, SystemLabelId};
use crate::models::{
    DraftAttachmentInternalError, DraftAttachmentMetadata, DraftSendFailure, DraftSendResult,
    DraftSendResultOrigin,
};
use crate::{AppError, MailContextError};
use mail_action_queue::action::{ActionGroup, WriterGuard, WriterGuardError};
use mail_core_api::services::proton::LabelId;
use mail_core_common::datatypes::LocalLabelId;
use mail_core_common::models::{Label, ModelExtension, ModelIdExtension};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::Tether;
use regex::Regex;
use tracing::error;

pub const SEND_ACTION_GROUP: ActionGroup = ActionGroup::new("MAIL_SEND");
pub const SHARE_EXT_ACTION_GROUP: ActionGroup = ActionGroup::new("SHARE_EXT");

async fn local_draft_label_id(tether: &Tether) -> Result<LocalLabelId, MailContextError> {
    let Some(local_draft_label_id) =
        Label::remote_id_counterpart(LabelId::drafts(), tether).await?
    else {
        return Err(AppError::RemoteLabelDoesNotExist(LabelId::drafts()).into());
    };

    Ok(local_draft_label_id)
}

async fn local_all_draft_label_id(tether: &Tether) -> Result<LocalLabelId, MailContextError> {
    let Some(local_all_draft_label_id) =
        Label::remote_id_counterpart(LabelId::all_drafts(), tether).await?
    else {
        return Err(AppError::RemoteLabelDoesNotExist(LabelId::all_drafts()).into());
    };

    Ok(local_all_draft_label_id)
}

async fn local_all_mail_label_id(tether: &Tether) -> Result<LocalLabelId, MailContextError> {
    let Some(local_all_mail_label_id) =
        Label::remote_id_counterpart(LabelId::all_mail(), tether).await?
    else {
        return Err(AppError::RemoteLabelDoesNotExist(LabelId::all_mail()).into());
    };

    Ok(local_all_mail_label_id)
}

async fn local_sent_label_id(tether: &Tether) -> Result<LocalLabelId, MailContextError> {
    let Some(local_draft_label_id) = Label::remote_id_counterpart(LabelId::sent(), tether).await?
    else {
        return Err(AppError::RemoteLabelDoesNotExist(LabelId::sent()).into());
    };

    Ok(local_draft_label_id)
}

async fn local_all_sent_label_id(tether: &Tether) -> Result<LocalLabelId, MailContextError> {
    let Some(local_all_sent_label_id) =
        Label::remote_id_counterpart(LabelId::all_sent(), tether).await?
    else {
        return Err(AppError::RemoteLabelDoesNotExist(LabelId::all_sent()).into());
    };

    Ok(local_all_sent_label_id)
}

async fn local_outbox_label_id(tether: &Tether) -> Result<LocalLabelId, MailContextError> {
    let Some(local_draft_label_id) =
        Label::remote_id_counterpart(LabelId::outbox(), tether).await?
    else {
        return Err(AppError::RemoteLabelDoesNotExist(LabelId::outbox()).into());
    };

    Ok(local_draft_label_id)
}

async fn local_all_scheduled_label_id(tether: &Tether) -> Result<LocalLabelId, MailContextError> {
    let Some(local_draft_label_id) =
        Label::remote_id_counterpart(LabelId::all_scheduled(), tether).await?
    else {
        return Err(AppError::RemoteLabelDoesNotExist(LabelId::all_scheduled()).into());
    };

    Ok(local_draft_label_id)
}

async fn save_attachment_error(
    message_id: LocalMessageId,
    attachment_id: LocalAttachmentId,
    origin: DraftSendResultOrigin,
    writer_guard: &mut WriterGuard<'_, UserDb>,
    error: &MailContextError,
) -> Result<(), WriterGuardError> {
    writer_guard
        .tx(async |tx| {
            let mut send_result = DraftSendResult::failure(
                message_id,
                origin,
                DraftSendFailure::from_mail_context_error(error),
            );

            send_result
                .save(tx)
                .await
                .inspect_err(|e| error!("Failed to save send result: {e:?}"))?;

            if let Some(mut attachment_metadata) =
                DraftAttachmentMetadata::find_by_id(attachment_id, tx).await?
            {
                if error.is_network_failure() {
                    attachment_metadata.set_offline_state();
                } else {
                    attachment_metadata.set_error_state(
                        DraftAttachmentInternalError::from_mail_context_error(origin, error),
                    );
                }
                attachment_metadata
                    .save(tx)
                    .await
                    .inspect_err(|e| error!("Failed to save draft attachment metadata: {e:?}"))?;
            }

            Ok(())
        })
        .await
}

fn sanitize_draft_subject(subject: &str) -> String {
    // Remove ascii control characters from the string and new lines
    static INVALID_CHARS_RE: OnceLock<(Regex, Regex)> = OnceLock::new();
    let (ascii_chars_re, new_lines_re) = INVALID_CHARS_RE.get_or_init(|| {
        (
            Regex::new("[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]").expect("This should not fail"),
            Regex::new("\r\n|\r|\n").expect("This should not fail"),
        )
    });

    let cleaned = ascii_chars_re.replace_all(subject, "");
    new_lines_re.replace_all(&cleaned, " ").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_subject_ascii_control_chars() {
        let invalid_chars = ('\x00'..='\x08')
            .chain(['\x0B', '\x0C', '\x7F'])
            .chain('\x0E'..='\x1F')
            .collect::<Vec<_>>();
        let subject = invalid_chars
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("A");

        let subject_sanitized = sanitize_draft_subject(&subject);

        for char in &invalid_chars {
            assert!(!subject_sanitized.contains(*char));
        }
    }

    #[test]
    fn sanitize_subject_line_endings() {
        let subject = "Hello\rWorld\nHow\r\nis life?";
        let subject_sanitized = sanitize_draft_subject(subject);
        assert_eq!(subject_sanitized, "Hello World How is life?");
    }
}
