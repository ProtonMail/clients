use anyhow::Context;
use async_trait::async_trait;
use core_event_loop::v6::{EventSource, EventSubscriber};
use core_event_loop::{EventSubscriberError, EventSubscriberResult, RefreshFlag};
use mail_action_queue::action::ActionGroup;
use mail_action_queue::rebase::RebaseChangeSet;
use mail_issue_reporter_service::{IssueLevel, issue_report_keys_from_error};
use mail_labels_common::Label;
use mail_shared_types::ModelExtension;
use std::collections::HashMap;
use tracing::{debug, error};

use crate::contact::Contact;
use crate::events::{
    ContactActionQueueContext, ContactEventSessionContext, ContactEventSourceV6,
    ContactEventStorageContext, ContactEventSubscriberError, ContactIssueReporterContext,
    ContactTaskSpawnerContext,
};

#[derive(Clone, Default)]
pub struct ContactEventV6Subscriber;

#[async_trait]
impl<Core, Ctx> EventSubscriber<Ctx, ContactEventSourceV6<Core>> for ContactEventV6Subscriber
where
    Core: EventSource,
    Ctx: ContactEventSessionContext
        + ContactEventStorageContext
        + ContactIssueReporterContext
        + ContactTaskSpawnerContext
        + ContactActionQueueContext,
{
    fn name(&self) -> &'static str {
        "contact-v6-subscriber"
    }

    async fn on_event(
        &self,
        ctx: &Ctx,
        event: &<ContactEventSourceV6<Core> as EventSource>::Event,
        cache: &mut <ContactEventSourceV6<Core> as EventSource>::Cache,
    ) -> EventSubscriberResult<()> {
        async {
            cache.fetch_event_data(event, ctx.get_contact_api()).await?;

            let mut tether = ctx.get_contact_stash().connection().await?;
            let mut changeset = RebaseChangeSet::default();
            tether
                .write_tx(async |tx| {
                    if let Some(events) = &event.labels {
                        debug!("Handling contact label event");
                        for event in events {
                            Label::handle_event(
                                tx,
                                &event.id,
                                event.action.into(),
                                cache.get_label_mut(&event.id),
                                &mut changeset,
                            )
                            .await?;
                        }
                    }
                    if let Some(events) = &event.contacts {
                        debug!("Handling contact event");
                        for event in events {
                            Contact::handle_event(
                                tx,
                                &event.id,
                                event.action.into(),
                                cache.get_contact_mut(&event.id),
                                &mut changeset,
                            )
                            .await?;
                        }
                    }

                    if let Err(e) = ctx
                        .get_contact_action_queue()
                        .rebase_in(ActionGroup::default(), &changeset, tx)
                        .await
                    {
                        error!("Failed to rebase changes: {e}");
                    }
                    Ok::<_, ContactEventSubscriberError>(())
                })
                .await
        }
        .await
        .inspect_err(|e| {
            if !e.is_retryable() {
                ctx.report_contacts_event_issue(
                    IssueLevel::Critical,
                    "Failed to apply contacts (v6) event".into(),
                    issue_report_keys_from_error(e),
                );
            }
        })
        .map_err(|e| -> Box<dyn EventSubscriberError> { Box::new(e) })
    }

    async fn on_refresh(
        &self,
        ctx: &Ctx,
        _: RefreshFlag,
        _: &mut <ContactEventSourceV6<Core> as EventSource>::Cache,
    ) -> EventSubscriberResult<()> {
        if let Err(e) = refresh_contacts(ctx).await {
            if !e.is_retryable() {
                ctx.report_contacts_event_issue(
                    IssueLevel::Critical,
                    "Failed to apply refresh contacts (v6)".into(),
                    issue_report_keys_from_error(e.as_ref()),
                );
            }
            return Err(e);
        }
        Ok(())
    }
}

macro_rules! join_task {
    ($name:tt, $description: expr) => {{
        match $name.await {
            Ok(Ok(value)) => value,

            Ok(Err(err)) => return Err(err.into()),

            Err(err) => {
                return if err.is_cancelled() {
                    Err(anyhow::anyhow!(
                        "The task `{}` was cancelled, we need to run refresh again",
                        $description
                    )
                    .into())
                } else {
                    Err(
                        anyhow::anyhow!("Failed to join download remote {}: `{err}`", $description)
                            .into(),
                    )
                };
            }
        }
    }};
}

#[tracing::instrument(skip_all)]
pub async fn refresh_contacts<Ctx>(ctx: &Ctx) -> EventSubscriberResult<()>
where
    Ctx: ContactEventSessionContext + ContactEventStorageContext + ContactTaskSpawnerContext,
{
    async {
        let api = ctx.get_contact_api().clone();
        let contacts = ctx.spawn_contact_task(async move { Contact::sync(&api).await });
        let api = ctx.get_contact_api().clone();
        let all_remote_labels =
            ctx.spawn_contact_task(async move { Label::fetch_contact_labels(&api).await });
        let mut tether = ctx.get_contact_stash().connection().await?;
        let mut all_local_labels: HashMap<_, _> = Label::all_contact_groups(&tether)
            .await?
            .into_iter()
            .map(|label| (label.remote_id.clone(), label))
            .collect();
        debug!(
            "Number of labels available localy: {}",
            all_local_labels.len()
        );
        let all_remote_labels = join_task!(all_remote_labels, "labels");
        debug!(
            "Number of labels available remotely: {}",
            all_remote_labels.len()
        );
        for remote_label in &all_remote_labels {
            all_local_labels.remove(&remote_label.remote_id);
        }

        let contacts = join_task!(contacts, "contacts");

        tether
            .sync_write_tx(move |tx| {
                Label::store_labels(tx, all_remote_labels).context("Failed to sync labels")?;

                for local_label_to_remove in all_local_labels.into_values() {
                    debug!(
                        "Removing label with remote_id {:?}",
                        local_label_to_remove.remote_id
                    );
                    local_label_to_remove.delete_sync(tx)?;
                }
                contacts.store(tx)?;

                Ok(())
            })
            .await
            .inspect_err(|e| {
                error!("Failed to update database entries while refreshing core: {e}");
            })?;

        Ok::<_, ContactEventSubscriberError>(())
    }
    .await
    .map_err(|e| -> Box<dyn EventSubscriberError> { Box::new(e) })
}
