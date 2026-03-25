mod account;
mod core;

pub use account::*;
use contacts_common::events::{
    ContactActionQueueContext, ContactEventSessionContext, ContactEventStorageContext,
    ContactIssueReporterContext, ContactTaskSpawnerContext,
};
pub use core::*;

use crate::UserContext;

impl ContactEventSessionContext for UserContext {
    fn get_contact_api(&self) -> &mail_core_api::session::Session {
        &self.session
    }
}

impl ContactEventStorageContext for UserContext {
    fn get_contact_stash(&self) -> &mail_stash::stash::Stash<mail_stash::UserDb> {
        &self.user_stash
    }
}

impl ContactTaskSpawnerContext for UserContext {
    fn spawn_contact_task<F>(&self, task: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future<Output: Send> + Send + 'static,
    {
        self.spawn(task)
    }
}

impl ContactIssueReporterContext for UserContext {
    fn report_contacts_event_issue(
        &self,
        level: mail_issue_reporter_service::IssueLevel,
        message: String,
        keys: mail_issue_reporter_service::IssueReportKeys,
    ) {
        self.issue_reporter_service().report(level, message, keys);
    }
}

impl ContactActionQueueContext for UserContext {
    fn get_contact_action_queue(&self) -> &mail_action_queue::queue::Queue<mail_stash::UserDb> {
        self.queue()
    }
}
