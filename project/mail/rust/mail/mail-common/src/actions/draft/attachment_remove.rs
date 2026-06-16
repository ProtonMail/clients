use crate::actions::draft::{
    DraftAttachmentActionDependencyKeyBuilderExt, SEND_ACTION_GROUP, save_attachment_error,
};
use crate::datatypes::{LocalAttachmentId, LocalMessageId};
use crate::draft::{AttachmentRemoveError, AttachmentUploadError};
use crate::models::{
    Attachment, AttachmentType, DraftAttachmentMetadata, DraftAttachmentOwnership, DraftMetadata,
    DraftSendResultOrigin, MetadataId,
};
use crate::{MailContextError, MailUserContext};
use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionGroup, ActionId, DefaultVersionConverter, Handler,
    Priority, Type,
};
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api::services::proton::ProtonMail;
use mail_core_api::consts::Mail;
use mail_core_api::service::{ApiErrorInfo, ApiServiceError};
use mail_core_api::session::Session;
use mail_core_common::actions::dependency_builder::ActionDependencyKeysBuilder;
use mail_core_common::models::ModelExtension;
use mail_stash::orm::Model as _;
use mail_stash::stash::{Tether, WriteTx};
use mail_stash::{UserDb, params};
use serde::{Deserialize, Serialize};
use std::sync::Weak;
use tracing::{debug, error, info};

#[derive(Serialize, Deserialize)]
pub struct AttachmentRemove {
    metadata_id: MetadataId,
    attachment_id: LocalAttachmentId,
    local_message_id: Option<LocalMessageId>,
}

impl AttachmentRemove {
    pub fn new(metadata_id: MetadataId, attachment_id: LocalAttachmentId) -> Self {
        Self {
            metadata_id,
            attachment_id,
            local_message_id: None,
        }
    }
    async fn local_message_id(
        &self,
        tether: &Tether,
    ) -> Result<Option<LocalMessageId>, MailContextError> {
        let Some(metadata) = DraftMetadata::find_by_id(self.metadata_id, tether)
            .await
            .inspect_err(|e| {
                error!("Failed to load draft metadata: {e:?}");
            })?
        else {
            error!("Could not find metadata {:?}", self.metadata_id);
            return Err(AttachmentUploadError::MetadataNotFound(self.metadata_id).into());
        };

        Ok(metadata.local_message_id)
    }
}

impl Action<UserDb> for AttachmentRemove {
    const TYPE: Type = Type("draft_attachment_remove");
    const GROUP: ActionGroup = SEND_ACTION_GROUP;
    const VERSION: u32 = 1;
    const PRIORITY: Priority = Priority::Normal;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = AttachmentRemoveHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailContextError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        ActionDependencyKeysBuilder::new()
            .with_draft_attachment_optional(self.metadata_id, self.attachment_id)
            .record_draft_attachment(self.metadata_id, self.attachment_id)
            .build()
    }
}

pub struct AttachmentRemoveHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for AttachmentRemoveHandler {
    type Action = AttachmentRemove;

    async fn apply_local(
        &self,
        action_id: ActionId,
        action: &mut Self::Action,
        tx: &WriteTx<'_>,
    ) -> Result<
        <Self::Action as Action<UserDb>>::LocalOutput,
        <Self::Action as Action<UserDb>>::Error,
    > {
        let Some(mut attachment_metadata) =
            DraftAttachmentMetadata::find_by_id(action.attachment_id, tx)
                .await
                .inspect_err(|e| error!("Failed to load draft attachment metadata: {e:?}"))?
        else {
            return Err(
                AttachmentRemoveError::AttachmentMetadataNotFound(action.attachment_id).into(),
            );
        };

        // find message
        let message_id = action.local_message_id(tx).await?;

        tracing::info!(
            "Removing attachment {} from draft {}",
            attachment_metadata.local_attachment_id,
            attachment_metadata.metadata_id
        );

        // remove attachment from message. It is possible the draft does not exist yet, but
        // the attachment metadata is present nonetheless.
        if let Some(message_id) = message_id {
            tx.execute(
                "DELETE FROM message_attachments_metadata WHERE local_message_id=? AND local_attachment_id=?",
                params![message_id, action.attachment_id],
            )
                .await
                .inspect_err(|e| error!("Failed to remove attachment from message metadata: {e}"))?;
            tx.execute(
                "DELETE FROM message_attachments WHERE local_message_id=? AND local_attachment_id=?",
                params![message_id, action.attachment_id],
            )
                .await
                .inspect_err(|e| error!("Failed to remove attachment from message: {e}"))?;
        }

        // update attachment action id
        attachment_metadata.deleted = true;
        attachment_metadata
            .save(tx)
            .await
            .inspect_err(|e| error!("Failed to save draft attachment metadata: {e:?}"))?;
        DraftAttachmentMetadata::track_action::<_, Self::Action>(
            action.attachment_id,
            action_id,
            tx,
        )
        .await?;

        action.local_message_id = message_id;
        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        // Restore undeleted status.
        if let Some(mut attachment_metadata) =
            DraftAttachmentMetadata::find_by_id(action.attachment_id, tx)
                .await
                .inspect_err(|e| error!("Failed to load draft attachment metadata: {e:?}"))?
        {
            attachment_metadata.deleted = false;
            attachment_metadata
                .save(tx)
                .await
                .inspect_err(|e| error!("Failed to save draft attachment metadata: {e:?}"))?;
        };
        // re-assign attachment to message.
        if let Some(message_id) = action.local_message_id {
            tx.execute(
                "INSERT OR IGNORE INTO message_attachments_metadata (local_message_id, local_attachment_id) VALUES(?,?)",
                params![message_id, action.attachment_id],
            )
                .await.inspect_err(|e| error!("Failed to assign attachment to message metadata: {e}"))?;
            tx.execute(
                "INSERT OR IGNORE INTO message_attachments (local_message_id, local_attachment_id) VALUES(?,?)",
                params![message_id, action.attachment_id],
            )
                .await.inspect_err(|e| error!("Failed to assign attachment to message: {e}"))?;
        }
        Ok(())
    }

    async fn apply_remote(
        &self,
        _: ActionId,
        action: &mut Self::Action,
    ) -> Result<
        <Self::Action as Action<UserDb>>::RemoteOutput,
        <Self::Action as Action<UserDb>>::Error,
    > {
        let ctx = self.ctx.upgrade().ok_or(MailContextError::LostContext)?;
        let mut tether = ctx.user_stash().connection();
        let result = action.apply_remote_impl(ctx.session(), &mut tether).await;

        if let Err(error) = &result {
            // Replace error only for internal reporting, the action queue needs the original error
            // to retry.
            if let Err(e) = save_attachment_error(
                action.local_message_id.expect("Should be set"),
                action.attachment_id,
                DraftSendResultOrigin::AttachmentRemove,
                &mut tether,
                error,
            )
            .await
            {
                error!("Failed to save attachment upload result: {e:?}");
            }
        }
        result
    }
    async fn rebase_local(
        &self,
        _: ActionId,
        _: &mut Self::Action,
        _: &RebaseChangeSet,
        _: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        Ok(())
    }
}

impl AttachmentRemove {
    async fn apply_remote_impl(
        &self,
        api: &Session,
        tether: &mut Tether<UserDb>,
    ) -> Result<(), MailContextError> {
        // check metadata to see if attachment is owned or inherited
        let Some(attachment_metadata) =
            DraftAttachmentMetadata::find_by_id(self.attachment_id, tether)
                .await
                .inspect_err(|e| error!("Failed to load draft attachment metadata: {e:?}"))?
        else {
            return Err(
                AttachmentRemoveError::AttachmentMetadataNotFound(self.attachment_id).into(),
            );
        };

        // if owned delete on the backend
        if matches!(
            attachment_metadata.ownership,
            DraftAttachmentOwnership::Owned
        ) && let Some(AttachmentType::Remote(Some(remote_id))) =
            Attachment::local_id_counterpart(self.attachment_id, tether).await?
        {
            info!("Deleting {remote_id:?}");

            match api.delete_attachment(remote_id).await {
                Ok(()) => {}
                Err(e) => {
                    error!("Failed to delete attachment on the server{e}");
                    match e {
                        ApiServiceError::UnprocessableEntity(
                            _,
                            Some(ApiErrorInfo { code, .. }),
                        ) if code == Mail::AttachmentDoesNotExist as u32 => {
                            tracing::warn!("Attachment does not exist on server");
                        }
                        ApiServiceError::UnprocessableEntity(
                            _,
                            Some(ApiErrorInfo {
                                error: Some(error), ..
                            }),
                        ) => return Err(AttachmentRemoveError::BadRequest(error).into()),
                        e => return Err(e.into()),
                    }
                }
            }
        }

        // Delete metadata & attachment record
        tether
            .write_tx::<_, _, <Self as Action<UserDb>>::Error>(async |tx: &WriteTx<'_>| {
                // If we own the attachment, delete it.
                if matches!(
                    attachment_metadata.ownership,
                    DraftAttachmentOwnership::Owned
                ) {
                    info!("Deleting {:?} locally", self.attachment_id);
                    Attachment::delete_by_id(self.attachment_id, tx)
                        .await
                        .inspect_err(|e| {
                            error!("Failed to delete attachment: {e:?}");
                        })?;
                }

                debug!("Deleting draft attachment metadata");
                attachment_metadata
                    .delete(tx)
                    .await
                    .inspect_err(|e| error!("Failed to delete draft attachment metadata: {e:?}"))?;

                Ok(())
            })
            .await
    }
}
