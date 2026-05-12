use crate::event_loop::event_subscriber::CoreEventSubscriberError;
use crate::event_loop::v6::CoreEventSourceV6;
use crate::models::{Address, ModelExtension, User};
use crate::services::event_loop_service::EventManagerContext;
use crate::{UserContext, join_task};
use anyhow::Context;
use async_trait::async_trait;
use core_event_loop::v6::{EventSource, EventSubscriber};
use core_event_loop::{EventSubscriberError, EventSubscriberResult, RefreshFlag};
use mail_action_queue::action::ActionGroup;
use mail_action_queue::rebase::RebaseChangeSet;
use mail_contacts_common::contact::Contact;
use mail_contacts_common::contact_group::ContactGroup;
use mail_issue_reporter_service::{IssueLevel, issue_report_keys_from_error};
use mail_stash::orm::Model;
use std::collections::HashMap;
use std::sync::Weak;
use tracing::{debug, error};

#[derive(Clone)]
pub struct CoreEventV6Subscriber(Weak<UserContext>);

impl From<Weak<UserContext>> for CoreEventV6Subscriber {
    fn from(value: Weak<UserContext>) -> Self {
        Self(value)
    }
}

#[async_trait]
impl EventSubscriber<EventManagerContext, CoreEventSourceV6> for CoreEventV6Subscriber {
    fn name(&self) -> &'static str {
        "core-v6-subscriber"
    }

    async fn on_event(
        &self,
        _: &EventManagerContext,
        event: &<CoreEventSourceV6 as EventSource>::Event,
        cache: &mut <CoreEventSourceV6 as EventSource>::Cache,
    ) -> EventSubscriberResult<()> {
        let ctx = self
            .0
            .upgrade()
            .context("Context is dead")
            .map_err(CoreEventSubscriberError::Other)
            .map_err(|e| -> Box<dyn EventSubscriberError> { Box::new(e) })?;
        async {
            cache.fetch_event_data(event, ctx.session()).await?;

            let mut tether = ctx.user_stash.connection();
            tether
                .write_tx(async |tx| {
                    if event.users.as_ref().is_some_and(|v| !v.is_empty()) {
                        debug!("Handling user event");
                        // Clear the crypto key cache as keys might have changed.
                        ctx.crypto_key_service().clear_cache();
                        if let Some(user) = cache.get_user_mut() {
                            let mut user: User = user.clone().into();
                            user.remote_id = Some(ctx.user_id().clone());
                            user.save(tx).await?;
                        }
                    }

                    if event.user_settings.as_ref().is_some_and(|v| !v.is_empty()) {
                        debug!("Handling user settings event");
                        if let Some(settings) = cache.get_user_settings_mut() {
                            settings.remote_id = Some(ctx.user_id().clone());
                            settings.save(tx).await?;
                        }
                    }

                    if let Some(events) = &event.addresses {
                        debug!("Handling address event");
                        // Clear the crypto key cache as keys might have changed.
                        ctx.crypto_key_service().clear_cache();
                        let mut changeset = RebaseChangeSet::default();
                        for event in events {
                            Address::handle_event(
                                tx,
                                &event.id,
                                event.action.into(),
                                cache.get_address_mut(&event.id),
                                &mut changeset,
                            )
                            .await?;
                        }

                        if let Err(e) = ctx
                            .queue()
                            .rebase_in(ActionGroup::default(), &changeset, tx)
                            .await
                        {
                            tracing::error!("Failed to rebase: {e}");
                        }
                    }

                    Ok::<_, CoreEventSubscriberError>(())
                })
                .await
        }
        .await
        .inspect_err(|e| {
            if !e.is_retryable() {
                ctx.issue_reporter_service().report(
                    IssueLevel::Critical,
                    "Failed to apply core (v6) event".into(),
                    issue_report_keys_from_error(e),
                );
            }
        })
        .map_err(|e| -> Box<dyn EventSubscriberError> { Box::new(e) })
    }

    async fn on_refresh(
        &self,
        _: &EventManagerContext,
        _: RefreshFlag,
        _: &mut <CoreEventSourceV6 as EventSource>::Cache,
    ) -> EventSubscriberResult<()> {
        let ctx = self
            .0
            .upgrade()
            .context("Context is dead")
            .map_err(CoreEventSubscriberError::Other)
            .map_err(|e| -> Box<dyn EventSubscriberError> { Box::new(e) })?;
        if let Err(e) = refresh_core_v6(&ctx).await {
            if !e.is_retryable() {
                ctx.issue_reporter_service().report(
                    IssueLevel::Critical,
                    "Failed to apply refresh (v6)".into(),
                    issue_report_keys_from_error(e.as_ref()),
                );
            }
            return Err(e);
        }
        Ok(())
    }
}

#[tracing::instrument(skip_all)]
pub async fn refresh_core_v6(ctx: &UserContext) -> EventSubscriberResult<()> {
    async {
        let api = ctx.session().clone();
        let all_remote_addresses = ctx.spawn(async move { Address::sync(&api).await });
        let api = ctx.session().clone();
        let user_and_settings = ctx.spawn(async move { User::sync_user_and_settings(&api).await });

        let mut tether = ctx.mail_stash().connection();
        let mut all_local_addresses: HashMap<_, _> = Address::all(&tether)
            .await?
            .into_iter()
            .map(|addr| (addr.remote_id.clone(), addr))
            .collect();

        debug!(
            "Number of addresses available localy: {}",
            all_local_addresses.len()
        );

        let all_remote_addresses = join_task!(all_remote_addresses, "addresses").inner();
        let user_and_settings = join_task!(user_and_settings, "user and settings");

        debug!(
            "Number of addresses available remotely: {}",
            all_remote_addresses.len()
        );
        for remote_label in &all_remote_addresses {
            all_local_addresses.remove(&remote_label.remote_id);
        }

        tether
            .write_tx::<_, _, CoreEventSubscriberError>(async |tx| {
                for local_address_to_remove in all_local_addresses.into_values() {
                    debug!(
                        "Removing address with remote_id {:?}",
                        local_address_to_remove.remote_id
                    );
                    local_address_to_remove.delete(tx).await?;
                }
                for mut remote_address in all_remote_addresses {
                    remote_address.save(tx).await?;
                }

                tx.sync_bridge(move |tx| {
                    user_and_settings.store(tx)?;
                    Ok(())
                })
                .await?;

                Ok(())
            })
            .await
            .inspect_err(|e| {
                error!("Failed to update database entries while refreshing core: {e}");
            })?;

        Ok::<_, CoreEventSubscriberError>(())
    }
    .await
    .map_err(|e| -> Box<dyn EventSubscriberError> { Box::new(e) })
}

#[tracing::instrument(skip_all)]
pub async fn refresh_core_v5(ctx: &UserContext) -> EventSubscriberResult<()> {
    async {
        let api = ctx.session().clone();
        let contacts = ctx.spawn(async move { Contact::sync_without_contact_groups(&api).await });
        let api = ctx.session().clone();
        let all_remote_addresses = ctx.spawn(async move { Address::sync(&api).await });
        let api = ctx.session().clone();
        let user_and_settings = ctx.spawn(async move { User::sync_user_and_settings(&api).await });
        let api = ctx.session().clone();
        let all_remote_contact_groups = ctx.spawn(async move { ContactGroup::fetch(&api).await });

        let mut tether = ctx.mail_stash().connection();
        let mut all_local_addresses: HashMap<_, _> = Address::all(&tether)
            .await?
            .into_iter()
            .map(|addr| (addr.remote_id.clone(), addr))
            .collect();
        let mut all_local_contact_groups: HashMap<_, _> = ContactGroup::all(&tether)
            .await?
            .into_iter()
            .map(|label| (label.remote_id.clone(), label))
            .collect();
        debug!(
            "Number of labels available localy: {}",
            all_local_contact_groups.len()
        );

        debug!(
            "Number of addresses available localy: {}",
            all_local_addresses.len()
        );

        let all_remote_addresses = join_task!(all_remote_addresses, "addresses").inner();
        let user_and_settings = join_task!(user_and_settings, "user and settings");
        let all_remote_contact_groups = join_task!(all_remote_contact_groups, "contact_groups");

        debug!(
            "Number of addresses available remotely: {}",
            all_remote_addresses.len()
        );
        for remote_label in &all_remote_addresses {
            all_local_addresses.remove(&remote_label.remote_id);
        }
        debug!(
            "Number of contact groups available remotely: {}",
            all_remote_contact_groups.len()
        );
        for cg in &all_remote_contact_groups {
            all_local_contact_groups.remove(&cg.remote_id);
        }

        let contacts = join_task!(contacts, "contacts");

        tether
            .write_tx::<_, _, CoreEventSubscriberError>(async |tx| {
                for local_address_to_remove in all_local_addresses.into_values() {
                    debug!(
                        "Removing address with remote_id {:?}",
                        local_address_to_remove.remote_id
                    );
                    local_address_to_remove.delete(tx).await?;
                }
                for mut remote_address in all_remote_addresses {
                    remote_address.save(tx).await?;
                }

                for mut cg in all_remote_contact_groups {
                    cg.save(tx)
                        .await
                        .map_err(|e| anyhow::Error::new(e).context("Failed to store labels"))?;
                }

                for local_contact_group_to_remove in all_local_contact_groups.into_values() {
                    debug!(
                        "Removing contact group with remote_id {:?}",
                        local_contact_group_to_remove.remote_id
                    );
                    local_contact_group_to_remove.delete(tx).await?;
                }

                tx.sync_bridge(move |tx| {
                    user_and_settings.store(tx)?;
                    contacts.store(tx)?;
                    Ok(())
                })
                .await?;

                Ok(())
            })
            .await
            .inspect_err(|e| {
                error!("Failed to update database entries while refreshing core: {e}");
            })?;

        Ok::<_, CoreEventSubscriberError>(())
    }
    .await
    .map_err(|e| -> Box<dyn EventSubscriberError> { Box::new(e) })
}
