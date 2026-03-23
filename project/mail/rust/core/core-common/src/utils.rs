use std::time::Instant;

use base64::{Engine, prelude::BASE64_STANDARD};
pub use mail_api_utils::{PaginateOptions, PaginateResponse};
pub use mail_avatar::{first_grapheme_uppercase, proton_color};
use mail_core_api::MAX_PAGE_ELEMENT_COUNT;
use proton_crypto::generate_secure_random_bytes;
use tokio::task::JoinSet;

/// This is a utility ergonomic trait as a shorthand for doing
/// `foo.into_iter().map(Into::into).collect::<Vec<_>>()`
pub trait MapVec<A> {
    fn map_vec(self) -> A;
}

impl<T: IntoIterator<Item = B>, A, B> MapVec<Vec<A>> for T
where
    B: Into<A>,
{
    fn map_vec(self) -> Vec<A> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<T: IntoIterator<Item = B>, A, B> MapVec<Option<Vec<A>>> for Option<T>
where
    B: Into<A>,
{
    fn map_vec(self) -> Option<Vec<A>> {
        self.map(MapVec::map_vec)
    }
}

const NONCE_SIZE: usize = 32;

/// Generate a random nonce for the sake of Content Security Policy.
/// <https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy#nonce-nonce_value/>
#[must_use]
pub fn generate_csp_nonce() -> String {
    let bytes: [u8; NONCE_SIZE] = generate_secure_random_bytes();
    BASE64_STANDARD.encode(bytes)
}

#[allow(async_fn_in_trait)]
pub trait Paginatable {
    type PaginateOptions: PaginateOptions + Clone + Send + 'static;
    type Response: PaginateResponse<Self::Output>;
    type Output: Sized + Send + 'static;
    type Error: Send + 'static;
    type API: Send + Clone + 'static;
    const NAME: &'static str;
    const DEFAULT_PAGE_SIZE: u64 = MAX_PAGE_ELEMENT_COUNT;

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
