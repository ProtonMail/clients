use std::num::NonZeroU32;

/// SlimAPI `Page` / `PageSize` query parameters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LtSlimApiPageQuery {
    page: Option<u32>,
    page_size: Option<NonZeroU32>,
}

impl LtSlimApiPageQuery {
    /// Sets optional `page` and/or `page_size`.
    pub const fn new(page: Option<u32>, page_size: Option<NonZeroU32>) -> Self {
        Self { page, page_size }
    }

    pub const fn page(&self) -> Option<u32> {
        self.page
    }

    pub const fn page_size(&self) -> Option<NonZeroU32> {
        self.page_size
    }
}
