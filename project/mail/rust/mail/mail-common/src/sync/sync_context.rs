use mail_action_queue::queue::Queue;
use mail_core_common::services::UserEventService;
use mail_network_monitor_service::NetworkStatusObserver;
use mail_search::MailSearchService;
use mail_stash::UserDb;
use mail_stash::stash::Stash;
use mail_task_service::BackgroundAwareTaskService;
use tokio_util::sync::CancellationToken;

use crate::{BackwardSyncApi, MailUserContext};

pub trait SyncContext: Send + Sync + 'static {
    type Api: BackwardSyncApi;

    fn stash(&self) -> &Stash<UserDb>;
    fn api(&self) -> &Self::Api;
    fn queue(&self) -> &Queue<UserDb>;
    fn search_service(&self) -> Option<&MailSearchService>;
    fn task_service(&self) -> &BackgroundAwareTaskService;
    fn user_event_service(&self) -> &UserEventService;
    fn network_status_observer(&self) -> NetworkStatusObserver;
    fn child_cancellation_token(&self) -> CancellationToken;
}

impl SyncContext for MailUserContext {
    type Api = mail_core_api::session::Session;

    fn stash(&self) -> &Stash<UserDb> {
        self.user_stash()
    }

    fn api(&self) -> &Self::Api {
        self.session()
    }

    fn queue(&self) -> &Queue<UserDb> {
        self.action_queue()
    }

    fn search_service(&self) -> Option<&MailSearchService> {
        self.search_service()
    }

    fn task_service(&self) -> &BackgroundAwareTaskService {
        self.mail_context().core_context().task_service()
    }

    fn user_event_service(&self) -> &UserEventService {
        self.user_event_service()
    }

    fn network_status_observer(&self) -> NetworkStatusObserver {
        self.network_monitor_service().network_status_observer()
    }

    fn child_cancellation_token(&self) -> CancellationToken {
        self.create_child_cancellation_token()
    }
}
