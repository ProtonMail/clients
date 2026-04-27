mod data_scroller_source;
mod hybrid_search;
#[cfg(feature = "foundation_search")]
mod local_search;
mod mail_scroller_state;
mod remote_source;

pub use self::data_scroller_source::*;
pub use self::hybrid_search::*;
#[cfg(feature = "foundation_search")]
pub use self::local_search::*;
pub use self::remote_source::*;
use crate::datatypes::SearchOptions;
use crate::datatypes::{ContextualConversation, LocalConversationId, LocalMessageId, ReadFilter};
use crate::mail_scroller::CategoryView;
use crate::models::Message;
use crate::traits::ScrollerEq;
use crate::{MailContextError, MailUserContext};
use mail_core_common::datatypes::LocalLabelId;
use mail_stash::orm::Model;
use std::hash::Hash;
use std::{fmt::Debug, future::Future};
use tokio::task::JoinHandle;

pub type MailPaginatorJoinHandle = Option<JoinHandle<Result<(), MailContextError>>>;

pub trait MailScrollerSource
where
    Self: Send + Sync + 'static,
{
    type Item: MailScrollerItem;

    /// Initialize the data source and retrieve up to `element_count` elements from the server.
    ///
    /// You can return an optional join handle that [`MailScroller`] will use on the first
    /// call to [`MailScroller::fetch_more()`] if you want to preload some data in
    /// a background task.
    fn initialize(
        &mut self,
        ctx: &MailUserContext,
        invalidate: flume::Sender<()>,
        category_view: CategoryView,
    ) -> impl Future<Output = Result<MailPaginatorJoinHandle, MailContextError>> + Send;

    /// Return the items that fall into range of the synced data.
    ///
    /// If some item is outside that range and known to us, it should not be included.
    fn visible_elements(
        &self,
        ctx: &MailUserContext,
    ) -> impl Future<Output = Result<Vec<Self::Item>, MailContextError>> + Send;

    /// Return the total number of items that fall into range of the synced data.
    ///
    /// If some item is outside that range and known to us, it should not be included.
    fn seen_count(
        &self,
        ctx: &MailUserContext,
    ) -> impl Future<Output = Result<u64, MailContextError>> + Send;

    /// Return the total number of items that fall into range of the synced data.
    ///
    /// If some item is outside that range and known to us, it should not be included.
    fn synced_total(
        &self,
        ctx: &MailUserContext,
    ) -> impl Future<Output = Result<u64, MailContextError>> + Send;

    /// Return the total number of items in the label.
    fn all_total(
        &self,
        ctx: &MailUserContext,
    ) -> impl Future<Output = Result<u64, MailContextError>> + Send;

    /// Return if there is more data available in the source.
    fn has_more(
        &self,
        ctx: &MailUserContext,
    ) -> impl Future<Output = Result<bool, MailContextError>> + Send;

    /// Sync the next section of data from the remote source which should return up to
    /// `element_count` results.
    ///
    /// This method can await until the data is fetched and should return the
    /// new elements that are valid in this interval as well as the new total.
    fn sync_next(
        &mut self,
        ctx: &MailUserContext,
    ) -> impl Future<Output = Result<(Vec<Self::Item>, MailPaginatorJoinHandle), MailContextError>> + Send;

    fn sync_new(
        &mut self,
        ctx: &MailUserContext,
    ) -> impl Future<Output = Result<MailPaginatorJoinHandle, MailContextError>> + Send;

    /// Atomically update one or more state dimensions and re-initialize the source.
    ///
    /// Pass `None` for any dimension that should remain unchanged.
    /// Sources that don't support category filtering (e.g. search) ignore `category_view`.
    fn change_state(
        &mut self,
        ctx: &MailUserContext,
        unread: Option<ReadFilter>,
        label: Option<LocalLabelId>,
        keywords: Option<SearchOptions>,
        category_view: Option<CategoryView>,
    ) -> impl Future<Output = Result<MailPaginatorJoinHandle, MailContextError>> + Send;

    fn clear(
        &mut self,
        ctx: &MailUserContext,
    ) -> impl Future<Output = Result<MailPaginatorJoinHandle, MailContextError>> + Send;

    fn watched_tables(&self) -> Vec<String>;

    fn category_view(&self) -> &CategoryView;
}

pub trait MailScrollerItem
where
    Self: Clone + Debug + ScrollerEq + Send + Sync + 'static,
{
    type Id: Clone + Copy + Debug + Hash + Eq + PartialEq + Send + Sync;

    // A bit more awkward name to avoid clashing with `Model::id()`
    fn item_id(&self) -> Self::Id;
}

impl MailScrollerItem for Message {
    type Id = LocalMessageId;

    fn item_id(&self) -> Self::Id {
        self.id()
    }
}

impl MailScrollerItem for ContextualConversation {
    type Id = LocalConversationId;

    fn item_id(&self) -> Self::Id {
        self.local_id
    }
}
