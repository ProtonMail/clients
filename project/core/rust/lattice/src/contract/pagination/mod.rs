mod lt_slim_api_page_query;
mod lt_slim_api_page_size_error;

pub use lt_slim_api_page_query::LtSlimApiPageQuery;
pub use lt_slim_api_page_size_error::LtSlimApiPageSizeError;

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU32;

    #[test]
    fn with_pagination_rejects_over_max() {
        let page_size = NonZeroU32::new(151).unwrap();
        let err = LtSlimApiPageQuery::<150>::new()
            .with_pagination(Some(0), Some(page_size))
            .unwrap_err();
        assert_eq!(err.max, 150);
        assert_eq!(err.requested, 151);
    }
}
