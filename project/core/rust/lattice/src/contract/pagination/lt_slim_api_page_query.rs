use std::num::NonZeroU32;

use super::LtSlimApiPageSizeError;

/// SlimAPI `Page` / `PageSize` query, serialized via [`LtSerdeQueryParams`](crate::contract::LtSerdeQueryParams).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtSlimApiPageQuery<const MAX_PAGE_SIZE: u32> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<NonZeroU32>,
}

impl<const MAX_PAGE_SIZE: u32> LtSlimApiPageQuery<MAX_PAGE_SIZE> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets optional `page` and/or `page_size`. Errors if `page_size` is `Some` and greater than
    /// `MAX_PAGE_SIZE` (inclusive bound).
    pub fn with_pagination(
        mut self,
        page: Option<u32>,
        page_size: Option<NonZeroU32>,
    ) -> Result<Self, LtSlimApiPageSizeError> {
        if let Some(size) = page_size {
            let requested = size.get();
            if requested > MAX_PAGE_SIZE {
                return Err(LtSlimApiPageSizeError {
                    max: MAX_PAGE_SIZE,
                    requested,
                });
            }
        }
        self.page = page;
        self.page_size = page_size;
        Ok(self)
    }
}
