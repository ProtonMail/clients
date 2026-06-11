use std::num::NonZeroU32;

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display)]
#[display("PageSize {requested} exceeds endpoint maximum {max}")]
pub struct LtSlimApiPageSizeError {
    pub max: NonZeroU32,
    pub requested: NonZeroU32,
}

impl std::error::Error for LtSlimApiPageSizeError {}
