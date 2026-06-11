use std::future::Future;

use super::{LtPaginable, LtPagination};

/// Fetches every page of a [`LtPaginable`] contract, calling `fetch_page` for each page.
///
/// Uses [`LtPaginable::MAX_PAGE_SIZE`] for `PageSize`. Stops when a page is partial or when
/// accumulated length reaches `Total` (if present).
pub async fn fetch_all_pages_with<Req, E, Fut>(
    req: Req,
    mut fetch_page: impl FnMut(&LtPagination<Req>) -> Fut,
) -> Result<Vec<Req::Item>, E>
where
    Req: LtPaginable + Clone,
    Fut: Future<Output = Result<Req::Response, E>>,
{
    let mut all = Vec::new();
    let mut current_page: u32 = 0;

    let mut paginated = req.with_pagination_max_page_size(current_page);

    loop {
        let res = fetch_page(&paginated).await?;

        let (total, items) = Req::page_items(res);

        let count = items.len();
        all.extend(items);

        if count < Req::MAX_PAGE_SIZE.get() as usize {
            break;
        }

        debug_assert_eq!(
            count,
            Req::MAX_PAGE_SIZE.get() as usize,
            "The backend returned more items than the max page size"
        );

        if let Some(total) = total
            && all.len() >= total as usize
        {
            debug_assert_eq!(
                total as usize,
                all.len(),
                "The backend returned more items than the total"
            );
            break;
        }

        current_page += 1;
        paginated = paginated.with_pagination_max_page_size(current_page);
    }

    Ok(all)
}
