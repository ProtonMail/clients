use contacts_api::ContactApi;
use core_event_loop::EventSubscriberError;
use core_event_loop::v6::{EventSource, EventSourceDependencyList};
use futures::StreamExt;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use itertools::Itertools;
use mail_api_labels::LabelApi;
use mail_core_api::consts::General;
use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::{Action, ContactId, ContactRootEventV6, LabelId};
use mail_core_api::session::Session;
use mail_labels_common::{Label, LabelError};
use mail_stash::stash::StashError;
use std::collections::HashMap;
use std::marker::PhantomData;
use tracing::{debug, error};

use crate::contact::Contact;

pub struct ContactEventSourceV6<Core: EventSource> {
    p: PhantomData<Core>,
}

impl<Core: EventSource> EventSource for ContactEventSourceV6<Core> {
    type Event = ContactRootEventV6;
    type Cache = ContactEventCache;

    fn name() -> &'static str {
        "contacts-v6"
    }

    fn dependencies() -> EventSourceDependencyList {
        EventSourceDependencyList::default().with::<Core>()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContactEventSubscriberError {
    #[error(transparent)]
    Api(#[from] ApiServiceError),
    #[error(transparent)]
    Stash(#[from] StashError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<LabelError> for ContactEventSubscriberError {
    fn from(err: LabelError) -> Self {
        match err {
            LabelError::API(e) => Self::Api(e),
            LabelError::Stash(e) => Self::Stash(e),
            err => Self::Other(err.into()),
        }
    }
}

impl EventSubscriberError for ContactEventSubscriberError {
    fn is_network_failure(&self) -> bool {
        match self {
            Self::Api(e) => e.is_network_failure(),
            Self::Stash(_) | Self::Other(_) => false,
        }
    }

    fn is_retryable(&self) -> bool {
        match self {
            Self::Api(e) => e.is_network_failure() || e.is_server_failure(),
            Self::Stash(StashError::ConnectionAcquireTimedOut) => true,
            Self::Stash(_) | Self::Other(_) => false,
        }
    }
}

#[derive(Default)]
pub struct ContactEventCache {
    contacts: HashMap<ContactId, Contact>,
    labels: HashMap<LabelId, Label>,
}

impl ContactEventCache {
    pub async fn fetch_event_data(
        &mut self,
        event: &ContactRootEventV6,
        session: &Session,
    ) -> Result<(), ContactEventSubscriberError> {
        let mut tasks = FuturesUnordered::new();
        if let Some(events) = &event.contacts {
            debug!("Fetching contacts");
            let mut contact_ids = Vec::with_capacity(events.len());
            for event in events {
                if event.action != Action::Delete && event.action != Action::UpdateFlags {
                    contact_ids.push(event.id.clone());
                }
            }

            self.fetch_contacts(&mut tasks, session, contact_ids);
        }

        if let Some(events) = &event.labels {
            debug!("Fetching contact labels");
            let mut label_ids = Vec::with_capacity(events.len());
            for event in events {
                if event.action != Action::Delete {
                    label_ids.push(event.id.clone());
                }
            }
            self.fetch_labels(&mut tasks, session, label_ids);
        }

        let mut first_err = None;
        while let Some(result) = tasks.next().await {
            match result {
                Ok(data) => data.apply(self),
                Err(e) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
            }
        }

        if let Some(e) = first_err {
            return Err(e.into());
        }

        Ok(())
    }
    fn fetch_contacts(
        &self,
        tasks: &mut FuturesUnordered<FutureTask>,
        session: &Session,
        ids: impl IntoIterator<Item = ContactId>,
    ) {
        tasks.extend(
            ids.into_iter()
                .filter(|id| !self.contacts.contains_key(id))
                .map(|id| -> FutureTask {
                    let session = session.clone();
                    Box::pin(async move {
                        match session.get_contact(id.clone()).await {
                            Ok(r) => Ok(FetchData::Contact(id, r.contact.into())),
                            Err(ApiServiceError::UnprocessableEntity(_, Some(api_error)))
                                if api_error.code == General::NotExists as u32 =>
                            {
                                Ok(FetchData::ContactDoesNotExist(id))
                            }
                            Err(e) => {
                                error!("Failed to fetch {id:?}: {e}");
                                Err(e)
                            }
                        }
                    })
                }),
        );
    }

    pub fn get_contact_mut(&mut self, id: &ContactId) -> Option<&mut Contact> {
        self.contacts.get_mut(id)
    }

    fn fetch_labels(
        &self,
        tasks: &mut FuturesUnordered<FutureTask>,
        session: &Session,
        ids: impl IntoIterator<Item = LabelId>,
    ) {
        const MAX_CURRENT_LABEL_REQUEST: usize = 50;
        tasks.extend(
            ids.into_iter()
                .filter(|id| !self.labels.contains_key(id))
                .chunks(MAX_CURRENT_LABEL_REQUEST)
                .into_iter()
                .map(|ids| -> FutureTask {
                    let session = session.clone();
                    let ids = ids.collect::<Vec<_>>();
                    Box::pin(async move {
                        session
                            .get_labels_by_ids(ids)
                            .await
                            .inspect_err(|e| error!("Failed to get contact labels: {e}"))
                            .map(|r| {
                                FetchData::Labels(r.labels.into_iter().map(Into::into).collect())
                            })
                    })
                }),
        );
    }

    pub fn get_label_mut(&mut self, id: &LabelId) -> Option<&mut Label> {
        self.labels.get_mut(id)
    }
}

type FutureTask = BoxFuture<'static, Result<FetchData, ApiServiceError>>;

enum FetchData {
    Contact(ContactId, Contact),
    ContactDoesNotExist(ContactId),
    Labels(Vec<Label>),
}

impl FetchData {
    fn apply(self, cache: &mut ContactEventCache) {
        match self {
            FetchData::Contact(id, contact) => {
                cache.contacts.insert(id, contact);
            }
            FetchData::ContactDoesNotExist(id) => {
                tracing::warn!("{id:?} no longer exists on server");
            }
            FetchData::Labels(labels) => {
                for label in labels {
                    cache
                        .labels
                        .insert(label.remote_id.clone().expect("Should be set"), label);
                }
            }
        }
    }
}
