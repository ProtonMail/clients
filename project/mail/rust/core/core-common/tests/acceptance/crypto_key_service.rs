use mail_core_api::services::proton::GetKeysAllResponse;
use mail_core_common::services::crypto_key_service::PublicAddressKeysResponseCache;
use mail_core_common::test_utils::test_context::TestContext;
use mail_core_key_manager::error::{KeyHandlingError, LoadingError};
use mail_core_key_manager::{PublicAddressKeyApiFetchPolicy, PublicAddressKeyContactFetchPolicy};
use proton_crypto::new_pgp_provider;
use proton_crypto_account::keys::{APIPublicAddressKeyGroup, APIPublicAddressKeys};
use std::io::ErrorKind;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request};

#[tokio::test]
async fn fetch_public_keys_requires_network_success() {
    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;

    mock_keys_failure(ctx.mock_server()).await;

    let pgp = new_pgp_provider();
    let email = "foo@params.com";

    let tether = user_ctx.mail_stash().connection().await.unwrap();

    let result = user_ctx
        .crypto_key_service()
        .load_with_tether(&user_ctx, &tether)
        .address_keys_for_email(
            &pgp,
            email,
            true,
            PublicAddressKeyApiFetchPolicy::RequireSync,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await;

    assert!(matches!(
        result,
        Err(KeyHandlingError::Loading(LoadingError::Api(_)))
    ));
}

#[tokio::test]
async fn fetch_public_keys_stores_in_cache() {
    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;

    let pgp = new_pgp_provider();
    let email = "foo@params.com";

    let response = APIPublicAddressKeys {
        address_keys: APIPublicAddressKeyGroup::default(),
        catch_all_keys: None,
        unverified_keys: None,
        warnings: vec![],
        proton_mx: true,
        is_proton: true,
    };

    ctx.mock_get_keys_all_with_internal_param(
        email,
        Some(true),
        GetKeysAllResponse {
            address_keys: APIPublicAddressKeyGroup::default(),
            catch_all_keys: None,
            is_proton: true,
            proton_mx: true,
            unverified_keys: None,
            warnings: vec![],
        },
    )
    .await;

    let tether = user_ctx.mail_stash().connection().await.unwrap();

    user_ctx
        .crypto_key_service()
        .load_with_tether(&user_ctx, &tether)
        .address_keys_for_email(
            &pgp,
            email,
            true,
            PublicAddressKeyApiFetchPolicy::RequireSync,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    let cached = PublicAddressKeysResponseCache::get(email.to_owned(), true, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cached.into_response(), response);
}

#[tokio::test]
async fn fetch_public_keys_loads_cached_version_when_network_fails() {
    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;
    mock_keys_failure(ctx.mock_server()).await;
    let email = "foo@params.com";
    let mut tether = user_ctx.mail_stash().connection().await.unwrap();
    tether
        .tx(async |tx| {
            let response = APIPublicAddressKeys {
                address_keys: APIPublicAddressKeyGroup::default(),
                catch_all_keys: None,
                unverified_keys: None,
                warnings: vec![],
                proton_mx: false,
                is_proton: false,
            };
            PublicAddressKeysResponseCache::store(email.to_owned(), true, response, tx).await
        })
        .await
        .unwrap();

    let pgp = new_pgp_provider();
    user_ctx
        .crypto_key_service()
        .load_with_tether(&user_ctx, &tether)
        .address_keys_for_email(
            &pgp,
            email,
            true,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();
}

async fn mock_keys_failure(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/core/v4/keys/all"))
        .respond_with_err(|_: &Request| {
            std::io::Error::new(ErrorKind::ConnectionReset, "connection reset")
        })
        .mount(server)
        .await;
}
