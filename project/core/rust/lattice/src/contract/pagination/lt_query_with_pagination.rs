use std::{borrow::Cow, collections::HashMap};

use crate::{LatticeError, LtRequestQueryParams, Sensitive};

use super::LtSlimApiPageQuery;

/// Composes filter query params with SlimAPI pagination (`Page` / `PageSize`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtQueryWithPagination<T: LtRequestQueryParams> {
    pub query: Option<T>,
    pub pagination_query: LtSlimApiPageQuery,
}

impl<T: LtRequestQueryParams> LtRequestQueryParams for LtQueryWithPagination<T> {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        let mut query = if let Some(query) = &self.query {
            query.to_query_params()?
        } else {
            HashMap::new()
        };
        if let Some(page) = self.pagination_query.page() {
            query.insert(Cow::Borrowed("Page"), Sensitive::new(page.to_string()));
        }
        if let Some(page_size) = self.pagination_query.page_size() {
            query.insert(
                Cow::Borrowed("PageSize"),
                Sensitive::new(page_size.get().to_string()),
            );
        }
        Ok(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LtNoQueryParams;
    use std::num::NonZeroU32;

    fn page_size(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).expect("non-zero")
    }

    #[test]
    fn pagination_only_uses_pascal_case_keys() {
        let q: LtQueryWithPagination<LtNoQueryParams> = LtQueryWithPagination {
            query: None,
            pagination_query: LtSlimApiPageQuery::new(Some(0), Some(page_size(150))),
        };
        let params = q.to_query_params().unwrap();
        assert!(params.contains_key("Page"));
        assert!(params.contains_key("PageSize"));
        assert!(!params.contains_key("page"));
    }

    #[test]
    fn merges_with_inner_query() {
        let q = LtQueryWithPagination {
            query: Some(LtNoQueryParams),
            pagination_query: LtSlimApiPageQuery::new(Some(1), Some(page_size(150))),
        };
        let params = q.to_query_params().unwrap();
        assert_eq!(
            params.get("Page").map(|v| v.clone().into_inner()),
            Some("1".to_string())
        );
        assert_eq!(params.len(), 2);
    }
}
