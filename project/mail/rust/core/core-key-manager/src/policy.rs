/// Controls whether the key manager uses a persistent cache when fetching other addresses' public
/// keys.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash)]
pub enum PublicAddressKeyApiFetchPolicy {
    #[default]
    /// Always fetch fresh data from the API; error out on failure.
    RequireSync,
    /// Use a stale cached value if the live API call fails.
    AllowCachedFallback,
}

/// Controls how the contact-based pinned-key lookup fetches its vCard data.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash)]
pub enum PublicAddressKeyContactFetchPolicy {
    #[default]
    /// Always sync the contact from the server; error out on failure.
    RequireSync,
    /// Use locally cached card data if the network request fails.
    AllowCachedFallback,
}

impl From<PublicAddressKeyContactFetchPolicy> for PublicAddressKeyApiFetchPolicy {
    fn from(value: PublicAddressKeyContactFetchPolicy) -> Self {
        match value {
            PublicAddressKeyContactFetchPolicy::RequireSync => Self::RequireSync,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback => Self::AllowCachedFallback,
        }
    }
}
