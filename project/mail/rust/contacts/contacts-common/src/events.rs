mod event_provider;
mod event_source;
mod event_store;
mod event_subscriber;

use std::sync::Arc;

pub use event_source::*;
pub use event_subscriber::*;
use mail_action_queue::queue::Queue;
use mail_api_session::session::Session;
use mail_issue_reporter_service::{IssueLevel, IssueReportKeys};
use mail_stash::UserDb;
use mail_stash::stash::Stash;
use tokio::task::JoinHandle;

pub const CONTACT_EVENT_TYPE_ID: &str = "proton-contact-event";

#[derive(Clone, Default)]
pub struct ContactEventLoopV6Context;

impl ContactEventLoopV6Context {
    #[must_use]
    pub fn boxed(&self) -> Box<Self> {
        Box::new(self.clone())
    }
}

pub trait ContactEventSessionContext: Send + Sync + 'static {
    fn get_contact_api(&self) -> &Session;
}

pub trait ContactEventStorageContext: Send + Sync + 'static {
    fn get_contact_stash(&self) -> &Stash<UserDb>;
}

pub trait ContactIssueReporterContext {
    fn report_contacts_event_issue(
        &self,
        level: IssueLevel,
        message: String,
        keys: IssueReportKeys,
    );
}

pub trait ContactTaskSpawnerContext {
    fn spawn_contact_task<F>(&self, task: F) -> JoinHandle<F::Output>
    where
        F: Future<Output: Send> + Send + 'static;
}

pub trait ContactActionQueueContext {
    fn get_contact_action_queue(&self) -> &Queue<UserDb>;
}

// Compatability helpers for UserContext integration

impl<T: ContactEventStorageContext> ContactEventStorageContext for Arc<T> {
    fn get_contact_stash(&self) -> &Stash<UserDb> {
        self.as_ref().get_contact_stash()
    }
}

impl<T: ContactEventSessionContext> ContactEventSessionContext for Arc<T> {
    fn get_contact_api(&self) -> &Session {
        self.as_ref().get_contact_api()
    }
}

impl<T: ContactActionQueueContext> ContactActionQueueContext for Arc<T> {
    fn get_contact_action_queue(&self) -> &Queue<UserDb> {
        self.as_ref().get_contact_action_queue()
    }
}

impl<T: ContactTaskSpawnerContext> ContactTaskSpawnerContext for Arc<T> {
    fn spawn_contact_task<F>(&self, task: F) -> JoinHandle<F::Output>
    where
        F: Future<Output: Send> + Send + 'static,
    {
        self.as_ref().spawn_contact_task(task)
    }
}

impl<T: ContactIssueReporterContext> ContactIssueReporterContext for Arc<T> {
    fn report_contacts_event_issue(
        &self,
        level: IssueLevel,
        message: String,
        keys: IssueReportKeys,
    ) {
        self.as_ref()
            .report_contacts_event_issue(level, message, keys);
    }
}
