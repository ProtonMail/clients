use crate::transport::LtTransportProvider;

use super::{LtPaginable, fetch_all_pages_with};

/// Extension for [`LtTransportProvider`] to fetch every page of a [`LtPaginable`] contract.
pub trait LtPaginationTransportExt: LtTransportProvider {
    /// Fetches all pages using [`LtPaginable::MAX_PAGE_SIZE`] for `PageSize`.
    fn fetch_all_pages<Req: LtPaginable + Clone + Send>(
        &self,
        req: Req,
    ) -> impl Future<Output = Result<Vec<Req::Item>, Self::Error>> {
        fetch_all_pages_with(req, move |paginated| {
            let paginated = paginated.clone();
            async move { self.send_contract_request(&paginated).await }
        })
    }
}

impl<T: LtTransportProvider> LtPaginationTransportExt for T {}
