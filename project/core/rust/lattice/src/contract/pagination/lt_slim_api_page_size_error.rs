/// `PageSize` exceeds the endpoint's [`MAX_PAGE_SIZE`](super::LtSlimApiPageQuery::with_pagination) bound.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display)]
#[display("PageSize {requested} exceeds endpoint maximum {max}")]
pub struct LtSlimApiPageSizeError {
    pub max: u32,
    pub requested: u32,
}

impl std::error::Error for LtSlimApiPageSizeError {}
