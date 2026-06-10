use crate::MailUserContext;
use crate::actions::{GenericLabelRelatedActionData, MailActionError, filter_responses};
use crate::datatypes::{LocalConversationId, RollbackItemType};
use crate::models::{Conversation, ConversationLabel, RollbackItem};
use itertools::Itertools;
use mail_action_queue::action::{
    Action, ActionDependencyKeys, ActionId, DefaultVersionConverter, Handler, Type,
};
use mail_action_queue::rebase::{RebaseChangeSet, RebaseKey};
use mail_api::services::proton::ProtonMail;
use mail_core_common::datatypes::{LocalLabelId, SystemLabel, UnixTimestamp};
use mail_core_common::models::ModelIdExtension;
use mail_stash::exports::ToSql;
use mail_stash::orm::Model;
use mail_stash::stash::WriteTx;
use mail_stash::utils::placeholders;
use mail_stash::{UserDb, params};
use serde::{self, Deserialize, Serialize};
use std::sync::Weak;
use tracing::error;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Unsnooze {
    action_data: GenericLabelRelatedActionData<Conversation>,
    conv_snooze_time: Vec<(LocalConversationId, UnixTimestamp)>,
}

impl Unsnooze {
    pub fn new(label_id: LocalLabelId, ids: impl IntoIterator<Item = LocalConversationId>) -> Self {
        Self {
            action_data: GenericLabelRelatedActionData::new(label_id, ids),
            conv_snooze_time: vec![],
        }
    }
}

impl Action<UserDb> for Unsnooze {
    const TYPE: Type = Type("unsnooze_conversations");
    const VERSION: u32 = 1;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = UnsnoozeHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = MailActionError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        self.action_data
            .snooze_unsnooze_action_dependency_keys()
            .build()
    }
}

pub struct UnsnoozeHandler {
    pub ctx: Weak<MailUserContext>,
}

impl Handler<UserDb> for UnsnoozeHandler {
    type Action = Unsnooze;

    async fn apply_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        tx: &WriteTx<'_>,
    ) -> Result<
        <Self::Action as Action<UserDb>>::LocalOutput,
        <Self::Action as Action<UserDb>>::Error,
    > {
        if action.action_data.data.target_ids.is_empty() {
            return Err(MailActionError::NoInput);
        }

        let mut parameters = action
            .action_data
            .data
            .target_ids
            .iter()
            .copied()
            .map(|i| Box::new(i) as Box<dyn ToSql + Send>)
            .collect_vec();
        let placeholders = placeholders(&parameters);
        parameters.push(Box::new(SystemLabel::Snoozed.remote_id()));
        let conv_labels = ConversationLabel::find(
            format!("WHERE local_conversation_id IN ({placeholders}) AND remote_label_id=?"),
            parameters,
            tx,
        )
        .await?;

        for conv_label in conv_labels {
            action.conv_snooze_time.push((
                conv_label
                    .local_conversation_id
                    .expect("Conversation label must have a conversation id"),
                conv_label.context_snooze_time,
            ));
        }

        Conversation::unsnooze(
            action.action_data.label_id,
            &action.action_data.data.target_ids,
            tx,
        )
        .await?;

        Ok(())
    }

    async fn revert_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        for (conv_id, snoozed_until) in action.conv_snooze_time.iter() {
            // we don't want to validate the previous snoozed state.
            Conversation::snooze_unchecked(&[*conv_id], *snoozed_until, tx).await?;
            // Resync conversation just in we are re-snoozing to some time in the past
            // or the snooze period has already ended.
            if let Some(api_conversation_id) =
                Conversation::local_id_counterpart(*conv_id, tx).await?
            {
                RollbackItem::new(
                    api_conversation_id.into_inner(),
                    RollbackItemType::Conversation,
                )
                .save(tx)
                .await?;
            }
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
        let ctx = self.ctx.upgrade().ok_or(MailActionError::LostContext)?;
        let mut tether = ctx.user_stash().connection();

        let (_, remote_target_ids) = action.action_data.resolve_ids_legacy(&tether).await?;

        if remote_target_ids.is_empty() {
            tracing::warn!(
                "No remote target ids to unsnooze, local only ids: {:?}",
                action.action_data.data.target_ids
            );
            return Ok(());
        }

        let response = ctx
            .session()
            .put_conversations_unsnooze(remote_target_ids)
            .await?;

        let responses = filter_responses(response.responses);

        if !responses.is_empty() {
            tether
                .write_tx::<_, _, <Self::Action as Action<UserDb>>::Error>(async |tx| {
                    error!("Unsnooze operation failed for: {:?}", responses);

                    for remote_id in responses {
                        RollbackItem::new(remote_id.to_string(), RollbackItemType::Conversation)
                            .save(tx)
                            .await?;
                    }

                    Ok(())
                })
                .await?;
        }

        Ok(())
    }

    async fn rebase_local(
        &self,
        _: ActionId,
        action: &mut Self::Action,
        changeset: &RebaseChangeSet,
        tx: &WriteTx<'_>,
    ) -> Result<(), <Self::Action as Action<UserDb>>::Error> {
        for id in &action.action_data.data.target_ids {
            let rebase_key: RebaseKey = (*id).into();

            if let Some(label) = ConversationLabel::find_first(
                "WHERE local_conversation_id=? AND remote_label_id=?",
                params![*id, SystemLabel::Snoozed.remote_id()],
                tx,
            )
            .await?
                && let Some((_, time)) = action
                    .conv_snooze_time
                    .iter_mut()
                    .find(|(conv_id, _)| *conv_id == *id)
            {
                *time = label.context_snooze_time;
            }

            if changeset.contains(&rebase_key) {
                Conversation::unsnooze(action.action_data.label_id, &[*id], tx).await?;
            }
        }
        Ok(())
    }
}
