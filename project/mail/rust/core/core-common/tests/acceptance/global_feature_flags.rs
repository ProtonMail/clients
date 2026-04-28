use std::sync::Arc;
use std::time::Duration;

use mail_core_api::services::proton::{
    GetUnleashFeaturesResponse, UnleashToggle, UnleashToggleVariant,
};
use mail_core_common::datatypes::UnixTimestamp;
use mail_core_common::device::{DeviceInfo, DeviceInfoProvider, DynDeviceInfoProvider};
use mail_core_common::models::FeatureFlag;
use mail_core_common::services::FeatureFlagsService;
use mail_core_common::test_utils::test_context::TestContext;
use mail_core_common::test_utils::utils::RespondNthTime;
use mail_stash::orm::Model;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn test_feature_flags_cold_start_background_fetch() {
    let ctx = TestContext::new().await;

    let mock_response = GetUnleashFeaturesResponse {
        toggles: vec![
            UnleashToggle {
                name: "TestFeatureA".to_string(),
                enabled: true,
                impression_data: false,
                variant: test_unleash_variant(),
            },
            UnleashToggle {
                name: "TestFeatureB".to_string(),
                enabled: true,
                impression_data: false,
                variant: test_unleash_variant(),
            },
        ],
    };

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
        .expect(1)
        .named("Cold start Unleash fetch")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();

    assert_eq!(feature_flags.get("TestFeatureA").await.unwrap(), None);
    assert_eq!(feature_flags.get("TestFeatureB").await.unwrap(), None);

    feature_flags.refresh().await.unwrap();

    wait_for_flag(feature_flags, "TestFeatureA", 4, 250).await;

    assert_eq!(feature_flags.get("TestFeatureA").await.unwrap(), Some(true));
    assert_eq!(feature_flags.get("TestFeatureB").await.unwrap(), Some(true));
    assert_eq!(feature_flags.get("NonExistentFeature").await.unwrap(), None);
}

#[tokio::test]
async fn test_feature_flags_warm_start_immediate_return() {
    let ctx = TestContext::new().await;

    {
        let past = UnixTimestamp::new(12);
        let mut tether = ctx
            .core_context()
            .account_stash()
            .connection()
            .await
            .unwrap();
        let mut cached_x = FeatureFlag {
            name: "CachedFeatureX".to_string(),
            enabled: true,
            modify_time: past,
        };
        let mut cached_y = FeatureFlag {
            name: "CachedFeatureY".to_string(),
            enabled: true,
            modify_time: past,
        };

        tether
            .write_tx(async move |tx| {
                cached_x.save(tx).await?;
                cached_y.save(tx).await
            })
            .await
            .unwrap();
    }

    let updated_response = GetUnleashFeaturesResponse {
        toggles: vec![
            UnleashToggle {
                name: "CachedFeatureX".to_string(),
                enabled: true,
                impression_data: false,
                variant: test_unleash_variant(),
            },
            UnleashToggle {
                name: "UpdatedFeatureZ".to_string(),
                enabled: true,
                impression_data: false,
                variant: test_unleash_variant(),
            },
        ],
    };

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(updated_response))
        .named("Background refresh")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();

    assert_eq!(
        feature_flags.get("CachedFeatureX").await.unwrap(),
        Some(true)
    );
    assert_eq!(
        feature_flags.get("CachedFeatureY").await.unwrap(),
        Some(true)
    );
    assert_eq!(feature_flags.get("UpdatedFeatureZ").await.unwrap(), None); // Not yet refreshed
}

#[tokio::test]
async fn test_feature_flags_warm_start_background_refresh() {
    let ctx = TestContext::new().await;

    {
        let past = UnixTimestamp::new(10);
        let mut tether = ctx
            .core_context()
            .account_stash()
            .connection()
            .await
            .unwrap();
        let mut existing_flag = FeatureFlag {
            name: "ExistingFeature".to_string(),
            enabled: true,
            modify_time: past,
        };

        tether
            .write_tx(async move |tx| existing_flag.save(tx).await)
            .await
            .unwrap();
    }

    let refresh_response = GetUnleashFeaturesResponse {
        toggles: vec![
            UnleashToggle {
                name: "ExistingFeature".to_string(),
                enabled: true,
                impression_data: false,
                variant: test_unleash_variant(),
            },
            UnleashToggle {
                name: "NewFeatureFromRefresh".to_string(),
                enabled: true,
                impression_data: false,
                variant: test_unleash_variant(),
            },
        ],
    };

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(refresh_response))
        .named("Background refresh with new flag")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();

    assert_eq!(
        feature_flags.get("ExistingFeature").await.unwrap(),
        Some(true)
    );
    assert_eq!(
        feature_flags.get("NewFeatureFromRefresh").await.unwrap(),
        None
    );

    feature_flags.refresh().await.unwrap();
    wait_for_flag(feature_flags, "NewFeatureFromRefresh", 10, 250).await;

    assert_eq!(
        feature_flags.get("ExistingFeature").await.unwrap(),
        Some(true)
    );
    assert_eq!(
        feature_flags.get("NewFeatureFromRefresh").await.unwrap(),
        Some(true)
    );

    {
        let tether = ctx
            .core_context()
            .account_stash()
            .connection()
            .await
            .unwrap();
        let existing_flag = FeatureFlag::by_name("ExistingFeature", &tether)
            .await
            .unwrap();
        let new_flag = FeatureFlag::by_name("NewFeatureFromRefresh", &tether)
            .await
            .unwrap();

        assert!(existing_flag.unwrap().enabled);
        assert!(new_flag.unwrap().enabled,);
    }
}

#[tokio::test]
async fn test_feature_flags_network_failure_preserves_cache() {
    let ctx = TestContext::new().await;

    {
        let past = UnixTimestamp::new(5);
        let mut tether = ctx
            .core_context()
            .account_stash()
            .connection()
            .await
            .unwrap();
        let mut cached_flag = FeatureFlag {
            name: "CachedFlag".to_string(),
            enabled: true,
            modify_time: past,
        };

        tether
            .write_tx(async move |tx| cached_flag.save(tx).await)
            .await
            .unwrap();
    }

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "Code": 500,
            "Error": "Internal server error"
        })))
        .named("Network failure")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();
    _ = feature_flags.refresh().await;

    assert_eq!(feature_flags.get("CachedFlag").await.unwrap(), Some(true));
    assert_eq!(feature_flags.get("NonExistentFlag").await.unwrap(), None);

    // To simulate that some time passed, we still get cached result.
    tokio::time::sleep(Duration::from_millis(1000)).await;

    assert_eq!(feature_flags.get("CachedFlag").await.unwrap(), Some(true));
}

#[tokio::test]
async fn test_feature_flags_handle_network_failure() {
    let ctx = TestContext::new().await;

    Mock::given(method("GET"))
        .and(path("/api/core/v4/tests/ping"))
        .respond_with(ResponseTemplate::new(200))
        .mount(ctx.mock_server())
        .await;

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(RespondNthTime::new(
            2,
            ResponseTemplate::new(500).set_body_json(json!({
                "Code": 500,
                "Error": "Internal server error"
            })),
            ResponseTemplate::new(200).set_body_json(GetUnleashFeaturesResponse {
                toggles: vec![UnleashToggle {
                    name: "TestFeatureRetry".to_string(),
                    enabled: true,
                    impression_data: false,
                    variant: test_unleash_variant(),
                }],
            }),
        ))
        .expect(3)
        .named("Cold start network failure then success")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();
    _ = feature_flags.refresh().await;

    wait_for_flag(feature_flags, "TestFeatureRetry", 10, 1000).await;

    assert_eq!(
        feature_flags.get("TestFeatureRetry").await.unwrap(),
        Some(true)
    );
    assert_eq!(feature_flags.get("NonExistentFeature").await.unwrap(), None);
}

struct MockDeviceInfoProvider {
    country: String,
}

#[async_trait::async_trait]
impl DeviceInfoProvider for MockDeviceInfoProvider {
    async fn get_device_info(&self) -> DeviceInfo {
        DeviceInfo {
            country: self.country.clone(),
            language: String::new(),
            timezone: String::new(),
            timezone_offset: 0,
            model: String::new(),
            brand: String::new(),
            codename: String::new(),
            uuid: String::new(),
            rooted: false,
            font_scale: String::new(),
            storage: 0.0,
            dark_mode: false,
            keyboards: vec![],
        }
    }
}

#[tokio::test]
async fn test_feature_flags_request_includes_user_country() {
    let provider: DynDeviceInfoProvider = Arc::new(MockDeviceInfoProvider {
        country: "CH".to_string(),
    });
    let ctx = TestContext::with_device_info_provider(provider).await;

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .and(wiremock::matchers::query_param("userCountry", "CH"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(GetUnleashFeaturesResponse { toggles: vec![] }),
        )
        .expect(1)
        .named("Unleash request with userCountry")
        .mount(ctx.mock_server())
        .await;

    ctx.context().feature_flags().refresh().await.unwrap();
}

async fn wait_for_flag(
    service: &FeatureFlagsService,
    key: &str,
    mut attempts: usize,
    sleep_ms: u64,
) {
    let initial = attempts;
    while attempts > 0 {
        if service.get(key).await.unwrap().is_some() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        attempts -= 1;
    }

    panic!("Flag {key} not found after {initial} attempts");
}

fn test_unleash_variant() -> UnleashToggleVariant {
    UnleashToggleVariant {
        name: "enabled".to_string(),
        feature_enabled: true,
        payload: None,
    }
}
