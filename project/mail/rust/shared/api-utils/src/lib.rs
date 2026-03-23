use std::time::Instant;

use tokio::task::JoinSet;

pub trait PaginateOptions {
    fn from_zero(size: u64) -> Self;
    #[must_use]
    fn with_page(self, page: u64) -> Self;
    fn size(&self) -> u64;
}

pub trait PaginateResponse<T> {
    fn total(&self) -> u64;
    fn items(self) -> Vec<T>;
}

#[allow(async_fn_in_trait)]
pub trait Paginatable {
    type PaginateOptions: PaginateOptions + Clone + Send + 'static;
    type Response: PaginateResponse<Self::Output>;
    type Output: Sized + Send + 'static;
    type Error: Send + 'static;
    type API: Send + Clone + 'static;
    const NAME: &'static str;
    const DEFAULT_PAGE_SIZE: u64 = 200;

    fn fetch(
        api: &Self::API,
        options: Self::PaginateOptions,
    ) -> impl std::future::Future<Output = Result<Self::Response, Self::Error>> + std::marker::Send;

    async fn fetch_all_filtered(
        api: &Self::API,
        options: Self::PaginateOptions,
    ) -> Result<Vec<Self::Output>, Self::Error> {
        // In order to maximize throughput we do as follows:
        // 1. We download the first batch
        // 2. We calculate how many batches are left and request them all in parallel.
        let t0 = Instant::now();

        let page_size = options.size();
        let first_response = Self::fetch(api, options.clone()).await?;

        tracing::debug!("Requested initial batch in {:?}", t0.elapsed());

        let mut joinset = JoinSet::new();
        if let Some(rem) = first_response.total().checked_sub(page_size) {
            let rem = rem.div_ceil(page_size);
            tracing::debug!("Requesting {rem} batches for {}", Self::NAME);
            for page in 1..=rem {
                let options = options.clone().with_page(page);
                let api = api.clone();
                joinset.spawn(async move {
                    Self::fetch(&api, options)
                        .await
                        .map(PaginateResponse::items)
                });
            }
        }

        let rest = joinset.join_all().await;

        let result: Vec<_> = std::iter::once(Ok(first_response.items()))
            .chain(rest)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();

        tracing::debug!(
            "Fetched {} {} in {:?}",
            result.len(),
            Self::NAME,
            t0.elapsed()
        );
        Ok(result)
    }

    async fn fetch_all(api: &Self::API) -> Result<Vec<Self::Output>, Self::Error> {
        Self::fetch_all_filtered(
            api,
            Self::PaginateOptions::from_zero(Self::DEFAULT_PAGE_SIZE),
        )
        .await
    }
}
