use std::num::NonZeroU32;

use super::{LtPagination, LtSlimApiPageQuery};
use crate::{LtContract, LtSlimApiPageSizeError};

/// Marker for list endpoints that support [`LtPagination`] wrapping.
pub trait LtPaginable: LtContract + Sized {
    type Item;
    const MAX_PAGE_SIZE: NonZeroU32;

    /// One SlimAPI page: `(total, items)` from [`LtContract::Response`].
    fn page_items(res: Self::Response) -> (Option<u32>, Vec<Self::Item>);

    fn with_pagination(
        self,
        page: u32,
        page_size: NonZeroU32,
    ) -> Result<LtPagination<Self>, LtSlimApiPageSizeError> {
        self.with_pagination_query(LtSlimApiPageQuery::new(Some(page), Some(page_size)))
    }

    /// Sets the page number and uses the max page size.
    fn with_pagination_max_page_size(self, page: u32) -> LtPagination<Self> {
        LtPagination::new_at_page_max_page_size(self, page)
    }

    fn with_pagination_query(
        self,
        pagination_query: LtSlimApiPageQuery,
    ) -> Result<LtPagination<Self>, LtSlimApiPageSizeError> {
        LtPagination::new(self, pagination_query)
    }
}
