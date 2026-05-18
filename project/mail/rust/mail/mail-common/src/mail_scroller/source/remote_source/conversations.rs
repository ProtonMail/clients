use super::{MailPaginatorJoinHandle, RemoteSource, utils};
use crate::datatypes::dependencies::DependencyFetcher;
use crate::datatypes::labels::{ScrollOrderDir, ScrollOrderField};
use crate::datatypes::{
    ContextualConversation, ConversationLabelsCount, DeletedItemType, ReadFilter,
};
use crate::models::{
    CanonicalCategory, Conversation, ConversationScrollData, DeletedItem, LabelExt, Message,
};
use crate::prefetch::PrefetchJob;
use crate::{MailContextError, MailUserContext};
use anyhow::anyhow;
use itertools::Itertools;
use mail_action_queue::action::ActionGroup;
use mail_action_queue::queue::Queue;
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api::services::proton::ProtonMail;
use mail_api::services::proton::common::ConversationId;
use mail_api::services::proton::prelude::{
    GetConversationsOptions, GetConversationsResponse, GetMessagesOptions,
};
use mail_api::services::proton::response_data::MessageMetadata as ApiMessageMetadata;
use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::LabelId;
use mail_core_api::session::Session;
use mail_core_common::datatypes::{LocalLabelId, UnixTimestamp};
use mail_core_common::models::{Label, ModelExtension};
use mail_stash::UserDb;
use mail_stash::stash::{Tether, WriteTx};
use std::ops::ControlFlow;
use tracing::{debug, error, info, instrument};

#[derive(Debug)]
pub(super) struct RemoteConversationScrollerSource;

impl RemoteSource for ConversationScrollData {
    fn sync_first_page(
        ctx: &MailUserContext,
        local_label_id: LocalLabelId,
        remote_label_ids: Vec<LabelId>,
        category: Vec<LocalLabelId>,
        unread: ReadFilter,
        page_size: usize,
        order_dir: ScrollOrderDir,
        order_field: ScrollOrderField,
        invalidate: Option<flume::Sender<()>>,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        let handle = ctx.spawn_ex(async move |ctx| {
            let items = RemoteConversationScrollerSource::sync_first_page(
                &ctx,
                local_label_id,
                remote_label_ids,
                category,
                unread,
                page_size,
                order_dir,
                order_field,
            )
            .await?;

            if let Some(invalidate) = invalidate
                && !items.is_empty()
            {
                invalidate.send_async(()).await.map_err(|e| {
                    MailContextError::Other(anyhow!(
                        "Could not notify about fetching first page: {e}"
                    ))
                })?;
            }

            let prefetch_jobs = items
                .into_iter()
                .filter(|item| !item.has_messages)
                .filter(|item| !item.deleted)
                .map(|item| PrefetchJob::Conversation(item.local_id, local_label_id))
                .collect();

            ctx.queue_prefetch_jobs(prefetch_jobs).await?;

            Ok(())
        });

        Ok(Some(handle))
    }

    fn sync_next_page(
        ctx: &MailUserContext,
        local_label_id: LocalLabelId,
        scroller: &Self,
        remote_label_ids: Vec<LabelId>,
        category: Vec<LocalLabelId>,
        unread: ReadFilter,
        page_size: usize,
        order_dir: ScrollOrderDir,
        order_field: ScrollOrderField,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        RemoteConversationScrollerSource::spawn_background_sync(
            ctx,
            scroller,
            local_label_id,
            remote_label_ids,
            category,
            unread,
            page_size,
            order_dir,
            order_field,
        )
    }

    fn sync_previous_page(
        ctx: &MailUserContext,
        local_label_id: LocalLabelId,
        scroller: &Self,
        remote_label_ids: Vec<LabelId>,
        category: Vec<LocalLabelId>,
        unread: ReadFilter,
        page_size: usize,
        order_dir: ScrollOrderDir,
        order_field: ScrollOrderField,
        sender: Option<flume::Sender<()>>,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        let remote_id = scroller.remote_conversation_id.clone();
        let context_time = scroller.context_time(order_field);

        let task = ctx.spawn_ex(async move |ctx| {
            let items = RemoteConversationScrollerSource::sync_previous_page(
                &ctx,
                local_label_id,
                remote_label_ids,
                remote_id,
                context_time,
                unread,
                category,
                page_size,
                order_dir,
                order_field,
            )
            .await?;

            if let Some(sender) = sender
                && !items.is_empty()
            {
                sender.send_async(()).await.map_err(|e| {
                    MailContextError::Other(anyhow!(
                        "Could not notify about fetching previous page: {e}"
                    ))
                })?;
            }

            Ok(())
        });

        Ok(Some(task))
    }
}

impl RemoteConversationScrollerSource {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn spawn_background_sync(
        ctx: &MailUserContext,
        scroller: &ConversationScrollData,
        label_local_id: LocalLabelId,
        remote_label_ids: Vec<LabelId>,
        category: Vec<LocalLabelId>,
        unread: ReadFilter,
        page_size: usize,
        order_dir: ScrollOrderDir,
        order_field: ScrollOrderField,
    ) -> Result<MailPaginatorJoinHandle, MailContextError> {
        let remote_id = scroller.remote_conversation_id.clone();
        let context_time = scroller.context_time(order_field);

        let task = ctx.spawn_ex(async move |ctx| {
            Self::sync_next_page(
                &ctx,
                label_local_id,
                remote_label_ids,
                remote_id,
                context_time,
                category,
                unread,
                page_size,
                order_dir,
                order_field,
            )
            .await?;

            Ok(())
        });

        Ok(Some(task))
    }

    #[instrument(skip_all)]
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn sync_first_page(
        ctx: &MailUserContext,
        local_label_id: LocalLabelId,
        remote_label_ids: Vec<LabelId>,
        category: Vec<LocalLabelId>,
        unread: ReadFilter,
        page_size: usize,
        order_dir: ScrollOrderDir,
        order_field: ScrollOrderField,
    ) -> Result<Vec<ContextualConversation>, MailContextError> {
        info!(
            ?local_label_id,
            ?remote_label_ids,
            ?unread,
            ?page_size,
            ?order_dir,
            ?order_field,
            "Syncing first page"
        );

        let (response, message_metadata) = fetch_conversations_and_messages(
            ctx.session(),
            GetConversationsOptions {
                label_id: Some(remote_label_ids.clone()),
                page_size: page_size as u64,
                unread: unread.into(),
                desc: order_dir.as_api_desc(),
                sort: order_field.as_api_sort(),
                ..Default::default()
            },
        )
        .await?;

        log_response(&response);

        let mut tether = ctx.user_stash().connection();

        // ---

        let ControlFlow::Continue(()) =
            utils::ensure_labels_are_idle(&mut tether, &remote_label_ids, &response.tasks_running)
                .await?
        else {
            return Ok(vec![]);
        };

        if response.conversations.is_empty() {
            return Ok(vec![]);
        }

        // ---

        let context_time = Self::context_time(&response, unread);

        let mut conversations: Vec<_> = response
            .conversations
            .into_iter()
            .map(|c| c.into())
            .collect();

        Self::save_conversations(
            local_label_id,
            &mut conversations,
            message_metadata,
            unread,
            context_time,
            true,
            order_dir,
            order_field,
            category,
            vec![],
            ctx.session(),
            &mut tether,
            ctx.action_queue(),
        )
        .await?;

        Ok(Self::contextual_conversations(
            local_label_id,
            conversations,
        ))
    }

    #[instrument(skip_all)]
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn sync_previous_page(
        ctx: &MailUserContext,
        local_label_id: LocalLabelId,
        remote_label_ids: Vec<LabelId>,
        first_element_id: ConversationId,
        first_element_time: UnixTimestamp,
        unread: ReadFilter,
        category: Vec<LocalLabelId>,
        page_size: usize,
        order_dir: ScrollOrderDir,
        order_field: ScrollOrderField,
    ) -> Result<Vec<ContextualConversation>, MailContextError> {
        // previous-page syncs do not advance the cursor, so category is not
        // threaded to save_conversations (update_scroller = false)
        info!(
            ?local_label_id,
            ?remote_label_ids,
            ?unread,
            ?page_size,
            ?order_dir,
            ?order_field,
            "Syncing previous page"
        );

        let (response, message_metadata) = fetch_conversations_and_messages(
            ctx.session(),
            GetConversationsOptions {
                anchor: Some(first_element_time.as_u64()),
                anchor_id: Some(first_element_id),
                label_id: Some(remote_label_ids.clone()),
                page_size: page_size as u64 + 1_u64,
                unread: unread.into(),
                desc: order_dir.reverse().as_api_desc(),
                sort: order_field.as_api_sort(),
                ..Default::default()
            },
        )
        .await?;

        log_response(&response);

        let mut tether = ctx.user_stash().connection();

        // ---

        let ControlFlow::Continue(()) =
            utils::ensure_labels_are_idle(&mut tether, &remote_label_ids, &response.tasks_running)
                .await?
        else {
            return Ok(vec![]);
        };

        if response.conversations.is_empty() {
            return Ok(vec![]);
        }

        // ---

        // Event though we are fetching messages, we do not need to fetch the message counters
        // as they are not displayed in conversation view mode.
        let conversation_label_counts = ctx
            .session()
            .get_conversations_count_for_labels(remote_label_ids.clone())
            .await?;

        let context_time = Self::context_time(&response, unread);

        let mut conversations: Vec<_> = response
            .conversations
            .into_iter()
            .map(|c| c.into())
            .collect();

        let conversation_counts = conversation_label_counts
            .counts
            .into_iter()
            .map_into()
            .collect();

        Self::save_conversations(
            local_label_id,
            &mut conversations,
            message_metadata,
            unread,
            context_time,
            false,
            order_dir,
            order_field,
            category,
            conversation_counts,
            ctx.session(),
            &mut tether,
            ctx.action_queue(),
        )
        .await?;

        Ok(Self::contextual_conversations(
            local_label_id,
            conversations,
        ))
    }

    #[instrument(skip_all)]
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn sync_next_page(
        ctx: &MailUserContext,
        local_label_id: LocalLabelId,
        remote_label_ids: Vec<LabelId>,
        last_element_id: ConversationId,
        last_element_time: UnixTimestamp,
        category: Vec<LocalLabelId>,
        unread: ReadFilter,
        page_size: usize,
        order_dir: ScrollOrderDir,
        order_field: ScrollOrderField,
    ) -> Result<Vec<ContextualConversation>, MailContextError> {
        info!(
            ?local_label_id,
            ?remote_label_ids,
            ?unread,
            ?page_size,
            ?order_dir,
            ?order_field,
            "Syncing next page"
        );

        let (mut response, message_metadata) = fetch_conversations_and_messages(
            ctx.session(),
            GetConversationsOptions {
                anchor: Some(last_element_time.as_u64()),
                anchor_id: Some(last_element_id.clone()),
                label_id: Some(remote_label_ids.clone()),
                page_size: page_size as u64 + 1_u64,
                unread: unread.into(),
                desc: order_dir.as_api_desc(),
                sort: order_field.as_api_sort(),
                ..Default::default()
            },
        )
        .await?;

        log_response(&response);

        let mut tether = ctx.user_stash().connection();

        // ---

        if !response.conversations.is_empty() {
            // Unless we are filtering, end id is always the first element in the returned
            // data, even if there is are no more elements.
            if response.conversations[0].id == last_element_id {
                response.conversations.remove(0);
            } else if response.conversations.len() > page_size {
                // sometimes we get more elements back than we need.
                response.conversations.pop();
            }
        }

        let ControlFlow::Continue(()) =
            utils::ensure_labels_are_idle(&mut tether, &remote_label_ids, &response.tasks_running)
                .await?
        else {
            return Ok(vec![]);
        };

        if response.conversations.is_empty() {
            return Ok(vec![]);
        }

        // ---

        let context_time = Self::context_time(&response, unread);

        let mut conversations: Vec<_> = response
            .conversations
            .into_iter()
            .map(|c| c.into())
            .collect();

        Self::save_conversations(
            local_label_id,
            &mut conversations,
            message_metadata,
            unread,
            context_time,
            true,
            order_dir,
            order_field,
            category,
            vec![],
            ctx.session(),
            &mut tether,
            ctx.action_queue(),
        )
        .await?;

        Ok(Self::contextual_conversations(
            local_label_id,
            conversations,
        ))
    }

    fn context_time(
        response: &GetConversationsResponse,
        unread: ReadFilter,
    ) -> Option<UnixTimestamp> {
        if unread != ReadFilter::All {
            // When filtering conversations, we need to use the contextual time
            // perform the next page query or the data will not be displayed
            // correctly.
            // This contextual time also does not match the ConversationLabel.context_time
            // we use to display the query results. This means that the data
            // will change after it is written to the database.
            response.conversations.last()?.context_time.map(Into::into)
        } else {
            None
        }
    }

    fn contextual_conversations(
        local_label_id: LocalLabelId,
        conversations: Vec<Conversation>,
    ) -> Vec<ContextualConversation> {
        conversations
            .into_iter()
            .filter_map(|c| ContextualConversation::new(c, local_label_id))
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_conversations(
        local_label_id: LocalLabelId,
        conversations: &mut [Conversation],
        message_metadata: Vec<ApiMessageMetadata>,
        unread: ReadFilter,
        context_time: Option<UnixTimestamp>,
        update_scroller: bool,
        order_dir: ScrollOrderDir,
        order_field: ScrollOrderField,
        category: Vec<LocalLabelId>,
        conversation_labels_count: Vec<ConversationLabelsCount>,
        api: &Session,
        tether: &mut Tether,
        queue: &Queue<UserDb>,
    ) -> Result<(), MailContextError> {
        // Resolve missing dependencies.
        let mut dependency_fetcher = DependencyFetcher::new();
        for conversation in conversations.iter() {
            dependency_fetcher
                .check_conversation(conversation, tether)
                .await?;
        }
        for message in &message_metadata {
            dependency_fetcher
                .check_api_message_metadata(message, tether)
                .await?;
        }
        let unresolved_label_ids = dependency_fetcher.fetch_and_store(api, tether).await?;
        for conversation in conversations.iter_mut() {
            conversation.prune_unresolved_labels(&unresolved_label_ids);
        }

        // We do not want to notify the UI about the not visible items
        // downloaded in the background hence `quiet_tx`
        tether
            .quiet_write_tx(async |tx| {
                if let Some(label) = Label::find_by_id(local_label_id, tx).await?
                    && label.is_busy(tx).await?
                {
                    return Ok(());
                }

                if !conversation_labels_count.is_empty() {
                    ConversationLabelsCount::upsert(conversation_labels_count.clone(), tx).await?;
                }

                // Batch check for deleted conversations
                let remote_ids = conversations
                    .iter()
                    .filter_map(|c| c.remote_id.as_ref().map(|id| id.as_str()));
                let deleted_ids = DeletedItem::find_deleted_by_remote_ids(
                    remote_ids,
                    DeletedItemType::Conversation,
                    tx,
                )
                .await?;

                let mut rebase_change_set = RebaseChangeSet::default();
                // Save all conversations.
                for conversation in conversations.iter_mut() {
                    use mail_stash::orm::Model;

                    // Skip conversations that have been deleted
                    if let Some(remote_id) = &conversation.remote_id
                        && deleted_ids.contains(&remote_id.to_string())
                    {
                        tracing::debug!(
                            "Skipping scrolled conversation {} - already deleted",
                            remote_id
                        );
                        continue;
                    }

                    // since we now fetch the messages, this should be set to true.
                    conversation.has_messages = true;
                    conversation.save(tx).await?;
                    rebase_change_set.add(conversation.id());
                }

                Message::save_scroller_messages(
                    message_metadata,
                    &mut rebase_change_set,
                    &unresolved_label_ids,
                    tx,
                )
                .await?;

                // We don't want this to cause failures in the scroller.
                if let Err(e) = queue
                    .rebase_in(ActionGroup::default(), &rebase_change_set, tx)
                    .await
                {
                    error!("Failed to rebase changes: {e}")
                }

                let Some((last, label)) = conversations
                    .iter()
                    .rev()
                    .filter_map(|conv| {
                        let conv_label = conv.label(local_label_id)?;
                        Some((conv, conv_label))
                    })
                    .next()
                else {
                    return Err(MailContextError::Other(anyhow!(
                        "There is no conversation with labels"
                    )));
                };

                let snooze_time = label.context_snooze_time;
                let context_time = context_time.unwrap_or(label.context_time);

                // Unwrap safety: RemoteId is present as this method is called on conversation
                // downloaded from API
                let remote_id = last.remote_id.clone().unwrap();
                let display_order = last.display_order;

                if update_scroller {
                    Self::update_scroller_data(
                        local_label_id,
                        remote_id,
                        unread,
                        context_time,
                        snooze_time,
                        display_order,
                        order_dir,
                        order_field,
                        &category,
                        tx,
                    )
                    .await?;
                }

                Ok(())
            })
            .await?;

        //TODO(ET-5589): This should not be necessary
        // Fake update to trigger the counters again in a tracking tx to see updates
        if !conversation_labels_count.is_empty()
            && let Err(e) = tether
                .write_tx(async |tx| ConversationLabelsCount::fake_update(local_label_id, tx).await)
                .await
        {
            error!("Failed to trigger fake label counters update: {e}");
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn update_scroller_data(
        local_label_id: LocalLabelId,
        remote_conv_id: ConversationId,
        unread: ReadFilter,
        context_time: UnixTimestamp,
        snooze_time: UnixTimestamp,
        display_order: u64,
        order_dir: ScrollOrderDir,
        order_field: ScrollOrderField,
        category: &[LocalLabelId],
        bond: &WriteTx<'_>,
    ) -> Result<ConversationScrollData, MailContextError> {
        debug!(
            "New conversation cursor {remote_conv_id} at time={context_time}, snooze_time={snooze_time}, order={display_order}"
        );

        let mut conv_paginator = ConversationScrollData::builder()
            .local_label_id(local_label_id)
            .unread(unread)
            .remote_conversation_id(remote_conv_id)
            .conversation_time(context_time)
            .snooze_time(snooze_time)
            .display_order(display_order)
            .order_dir(order_dir)
            .order_field(order_field)
            .category(CanonicalCategory::new(category.to_vec()))
            .build();

        conv_paginator.save(bond).await?;

        Ok(conv_paginator)
    }
}

fn log_response(response: &GetConversationsResponse) {
    debug!(
        "Fetched {}/{} {} elements",
        response.conversations.len(),
        response.total,
        if response.stale { "stale" } else { "fresh" }
    );
}

async fn fetch_conversations_and_messages(
    session: &Session,
    options: GetConversationsOptions,
) -> Result<(GetConversationsResponse, Vec<ApiMessageMetadata>), ApiServiceError> {
    const MESSAGE_PAGE_SIZE: u64 = 100;

    let conversations_response = session.get_conversations(options).await?;

    let conversation_ids = conversations_response
        .conversations
        .iter()
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();

    if conversation_ids.is_empty() {
        return Ok((conversations_response, vec![]));
    }

    let mut messages = Vec::new();
    let mut page_index = 0_u64;

    loop {
        debug!("Fetching conversations messages (page={})", page_index);

        let messages_response = session
            .get_messages(GetMessagesOptions {
                conversation_id: Some(conversation_ids.clone()),
                page: page_index,
                page_size: MESSAGE_PAGE_SIZE,
                ..Default::default()
            })
            .await?;

        debug!("Done fetching conversations messages (page={})", page_index);

        let was_empty = messages.is_empty();

        messages.extend(messages_response.messages);

        // if the returned messages is equal to the total, we can early exit, since we
        // fetched all messages. If not, the total will decrease on every subsequent page.
        if was_empty || messages.len() as u64 == messages_response.total {
            break;
        }

        page_index += 1;
    }

    Ok((conversations_response, messages))
}
