use std::{borrow::Cow, collections::HashMap, hash::Hasher};

use derive_more::{Deref, DerefMut};

use crate::{
    AuthReq, LatticeError, LtContract, LtPaginable, LtQueryWithPagination, LtSlimApiPageQuery,
    LtSlimApiPageSizeError, Method, Sensitive, UnauthReq,
};

/// Wraps a [`LtPaginable`] contract and injects `Page` / `PageSize` on the wire query.
#[derive(Deref, DerefMut)]
pub struct LtPagination<T: LtPaginable> {
    #[deref]
    #[deref_mut]
    contract: T,
    pagination_query: LtSlimApiPageQuery,
}

impl<T: LtPaginable + std::fmt::Debug> std::fmt::Debug for LtPagination<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LtPagination")
            .field("contract", &self.contract)
            .field("pagination_query", &self.pagination_query)
            .finish()
    }
}

impl<T: LtPaginable + PartialEq> PartialEq for LtPagination<T> {
    fn eq(&self, other: &Self) -> bool {
        self.contract == other.contract && self.pagination_query == other.pagination_query
    }
}

impl<T: LtPaginable + Eq> Eq for LtPagination<T> {}

impl<T: LtPaginable + std::hash::Hash> std::hash::Hash for LtPagination<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.contract.hash(state);
        self.pagination_query.hash(state);
    }
}

impl<T: LtPaginable + Clone> Clone for LtPagination<T> {
    fn clone(&self) -> Self {
        Self {
            contract: self.contract.clone(),
            pagination_query: self.pagination_query,
        }
    }
}

impl<T: LtPaginable> LtPagination<T> {
    /// Creates a new pagination with the given contract and pagination query.
    /// Will return an error if the page size is greater than the max page size.
    pub fn new(
        contract: T,
        pagination_query: LtSlimApiPageQuery,
    ) -> Result<Self, LtSlimApiPageSizeError> {
        if let Some(size) = pagination_query.page_size()
            && size > T::MAX_PAGE_SIZE
        {
            return Err(LtSlimApiPageSizeError {
                max: T::MAX_PAGE_SIZE,
                requested: size,
            });
        }
        Ok(Self {
            contract,
            pagination_query,
        })
    }

    /// Creates a new pagination at the given page.
    /// Will use the max page size for the page size.
    pub fn new_at_page_max_page_size(contract: T, page: u32) -> Self {
        Self {
            contract,
            pagination_query: LtSlimApiPageQuery::new(Some(page), Some(T::MAX_PAGE_SIZE)),
        }
    }

    pub fn into_inner(self) -> T {
        self.contract
    }

    pub fn pagination(&self) -> LtSlimApiPageQuery {
        self.pagination_query
    }

    /// Creates a new pagination at the given page.
    /// Will use the max page size for the page size.
    pub fn with_pagination_max_page_size(self, page: u32) -> Self {
        Self::new_at_page_max_page_size(self.into_inner(), page)
    }
}

impl<T: LtPaginable> LtContract for LtPagination<T> {
    type Response = T::Response;

    type Body<'b>
        = T::Body<'b>
    where
        Self: 'b;

    type Query<'q>
        = LtQueryWithPagination<T::Query<'q>>
    where
        Self: 'q;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        self.contract.path()
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        Some(LtQueryWithPagination {
            query: self.contract.query(),
            pagination_query: self.pagination_query,
        })
    }

    fn headers(&self) -> Result<HashMap<String, Sensitive<String>>, LatticeError> {
        self.contract.headers()
    }

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        self.contract.method()
    }
}

impl<T> AuthReq for LtPagination<T> where T: LtPaginable + AuthReq {}

impl<T> UnauthReq for LtPagination<T> where T: LtPaginable + UnauthReq {}
