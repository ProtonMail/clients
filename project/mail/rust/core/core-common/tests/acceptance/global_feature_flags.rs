use std::sync::Arc;
use std::time::Duration;

use mail_core_api::services::proton::{
    GetUnleashFeaturesResponse, UnleashToggle, UnleashTogglePayload, UnleashTogglePayloadType,
    UnleashToggleVariant,
};
use mail_core_common::datatypes::{FeatureFlagPayloadType, UnixTimestamp, Variant, VariantPayload};
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
        let mut tether = ctx.core_context().account_stash().connection();
        let mut cached_x = FeatureFlag {
            name: "CachedFeatureX".to_string(),
            enabled: true,
            modify_time: past,
            variant_name: None,
            variant_enabled: None,
            variant_payload_type: None,
            variant_payload_value: None,
        };
        let mut cached_y = FeatureFlag {
            name: "CachedFeatureY".to_string(),
            enabled: true,
            modify_time: past,
            variant_name: None,
            variant_enabled: None,
            variant_payload_type: None,
            variant_payload_value: None,
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
        let mut tether = ctx.core_context().account_stash().connection();
        let mut existing_flag = FeatureFlag {
            name: "ExistingFeature".to_string(),
            enabled: true,
            modify_time: past,
            variant_name: None,
            variant_enabled: None,
            variant_payload_type: None,
            variant_payload_value: None,
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
        let tether = ctx.core_context().account_stash().connection();
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
        let mut tether = ctx.core_context().account_stash().connection();
        let mut cached_flag = FeatureFlag {
            name: "CachedFlag".to_string(),
            enabled: true,
            modify_time: past,
            variant_name: None,
            variant_enabled: None,
            variant_payload_type: None,
            variant_payload_value: None,
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
        enabled: true,
        feature_enabled: true,
        payload: None,
    }
}

fn variant_named(name: &str) -> UnleashToggleVariant {
    UnleashToggleVariant {
        name: name.to_string(),
        enabled: true,
        feature_enabled: true,
        payload: None,
    }
}

fn variant_with_payload(
    name: &str,
    ty: UnleashTogglePayloadType,
    value: &str,
) -> UnleashToggleVariant {
    UnleashToggleVariant {
        name: name.to_string(),
        enabled: true,
        feature_enabled: true,
        payload: Some(UnleashTogglePayload {
            ty,
            value: value.to_string(),
        }),
    }
}

#[tokio::test]
async fn test_feature_flags_variant_persists() {
    let ctx = TestContext::new().await;

    let mock_response = GetUnleashFeaturesResponse {
        toggles: vec![UnleashToggle {
            name: "VariantFeature".to_string(),
            enabled: true,
            impression_data: false,
            variant: variant_with_payload("foo", UnleashTogglePayloadType::Json, r#"{"bar":1}"#),
        }],
    };

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
        .named("Variant persists fetch")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();
    feature_flags.refresh().await.unwrap();
    wait_for_flag(feature_flags, "VariantFeature", 10, 100).await;

    let variant = feature_flags
        .get_feature_flag_variant("VariantFeature")
        .await
        .unwrap();
    assert_eq!(
        variant,
        Some(Variant {
            name: "foo".to_string(),
            enabled: true,
            payload: Some(VariantPayload {
                ty: FeatureFlagPayloadType::Json,
                value: r#"{"bar":1}"#.to_string(),
            }),
        })
    );
}

#[tokio::test]
async fn test_feature_flags_variant_without_payload() {
    let ctx = TestContext::new().await;

    let mock_response = GetUnleashFeaturesResponse {
        toggles: vec![UnleashToggle {
            name: "NamedOnly".to_string(),
            enabled: true,
            impression_data: false,
            variant: variant_named("just-a-name"),
        }],
    };

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
        .named("Variant without payload")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();
    feature_flags.refresh().await.unwrap();
    wait_for_flag(feature_flags, "NamedOnly", 10, 100).await;

    let variant = feature_flags
        .get_feature_flag_variant("NamedOnly")
        .await
        .unwrap();
    assert_eq!(
        variant,
        Some(Variant {
            name: "just-a-name".to_string(),
            enabled: true,
            payload: None,
        })
    );
}

#[tokio::test]
async fn test_feature_flags_variant_changes_between_refreshes() {
    let ctx = TestContext::new().await;

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(RespondNthTime::new(
            1,
            ResponseTemplate::new(200).set_body_json(GetUnleashFeaturesResponse {
                toggles: vec![UnleashToggle {
                    name: "Mutating".to_string(),
                    enabled: true,
                    impression_data: false,
                    variant: variant_with_payload(
                        "alpha",
                        UnleashTogglePayloadType::String,
                        "first",
                    ),
                }],
            }),
            ResponseTemplate::new(200).set_body_json(GetUnleashFeaturesResponse {
                toggles: vec![UnleashToggle {
                    name: "Mutating".to_string(),
                    enabled: true,
                    impression_data: false,
                    variant: variant_with_payload(
                        "beta",
                        UnleashTogglePayloadType::String,
                        "second",
                    ),
                }],
            }),
        ))
        .named("Variant changes")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();

    feature_flags.refresh().await.unwrap();
    wait_for_flag(feature_flags, "Mutating", 10, 100).await;
    let first = feature_flags
        .get_feature_flag_variant("Mutating")
        .await
        .unwrap();
    assert_eq!(
        first,
        Some(Variant {
            name: "alpha".to_string(),
            enabled: true,
            payload: Some(VariantPayload {
                ty: FeatureFlagPayloadType::String,
                value: "first".to_string(),
            }),
        })
    );

    feature_flags.refresh().await.unwrap();
    let second = feature_flags
        .get_feature_flag_variant("Mutating")
        .await
        .unwrap();
    assert_eq!(
        second,
        Some(Variant {
            name: "beta".to_string(),
            enabled: true,
            payload: Some(VariantPayload {
                ty: FeatureFlagPayloadType::String,
                value: "second".to_string(),
            }),
        })
    );
}

#[tokio::test]
async fn test_feature_flags_variant_disappears_when_toggle_drops_out() {
    let ctx = TestContext::new().await;

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(RespondNthTime::new(
            1,
            ResponseTemplate::new(200).set_body_json(GetUnleashFeaturesResponse {
                toggles: vec![UnleashToggle {
                    name: "Vanishing".to_string(),
                    enabled: true,
                    impression_data: false,
                    variant: variant_with_payload("x", UnleashTogglePayloadType::Number, "42"),
                }],
            }),
            ResponseTemplate::new(200)
                .set_body_json(GetUnleashFeaturesResponse { toggles: vec![] }),
        ))
        .named("Toggle drops out")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();

    feature_flags.refresh().await.unwrap();
    wait_for_flag(feature_flags, "Vanishing", 10, 100).await;
    assert_eq!(feature_flags.get("Vanishing").await.unwrap(), Some(true));

    feature_flags.refresh().await.unwrap();
    assert_eq!(feature_flags.get("Vanishing").await.unwrap(), Some(false));
    let variant = feature_flags
        .get_feature_flag_variant("Vanishing")
        .await
        .unwrap();
    assert_eq!(variant, None);
}

#[tokio::test]
async fn test_feature_flags_variant_payload_types_round_trip() {
    let ctx = TestContext::new().await;

    let mock_response = GetUnleashFeaturesResponse {
        toggles: vec![
            UnleashToggle {
                name: "AsJson".to_string(),
                enabled: true,
                impression_data: false,
                variant: variant_with_payload("v", UnleashTogglePayloadType::Json, r#"{"a":1}"#),
            },
            UnleashToggle {
                name: "AsCsv".to_string(),
                enabled: true,
                impression_data: false,
                variant: variant_with_payload("v", UnleashTogglePayloadType::Csv, "a,b,c"),
            },
            UnleashToggle {
                name: "AsString".to_string(),
                enabled: true,
                impression_data: false,
                variant: variant_with_payload("v", UnleashTogglePayloadType::String, "hello"),
            },
            UnleashToggle {
                name: "AsNumber".to_string(),
                enabled: true,
                impression_data: false,
                variant: variant_with_payload("v", UnleashTogglePayloadType::Number, "7"),
            },
        ],
    };

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
        .named("All payload types")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();
    feature_flags.refresh().await.unwrap();
    wait_for_flag(feature_flags, "AsNumber", 10, 100).await;

    let cases = [
        ("AsJson", FeatureFlagPayloadType::Json, r#"{"a":1}"#),
        ("AsCsv", FeatureFlagPayloadType::Csv, "a,b,c"),
        ("AsString", FeatureFlagPayloadType::String, "hello"),
        ("AsNumber", FeatureFlagPayloadType::Number, "7"),
    ];
    for (name, ty, value) in cases {
        let variant = feature_flags.get_feature_flag_variant(name).await.unwrap();
        assert_eq!(
            variant,
            Some(Variant {
                name: "v".to_string(),
                enabled: true,
                payload: Some(VariantPayload {
                    ty,
                    value: value.to_string(),
                }),
            }),
            "round-trip failure for {name}"
        );
    }
}

#[tokio::test]
async fn test_feature_flags_variant_feature_disabled() {
    let ctx = TestContext::new().await;

    let mock_response = GetUnleashFeaturesResponse {
        toggles: vec![UnleashToggle {
            name: "DisabledFeature".to_string(),
            enabled: true,
            impression_data: false,
            variant: UnleashToggleVariant {
                name: "treatment".to_string(),
                enabled: true,
                feature_enabled: false,
                payload: Some(UnleashTogglePayload {
                    ty: UnleashTogglePayloadType::String,
                    value: "hello".to_string(),
                }),
            },
        }],
    };

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
        .named("Feature disabled, variant still set")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();
    feature_flags.refresh().await.unwrap();

    assert_eq!(
        feature_flags.get("DisabledFeature").await.unwrap(),
        Some(false)
    );
    let variant = feature_flags
        .get_feature_flag_variant("DisabledFeature")
        .await
        .unwrap();
    assert_eq!(
        variant,
        Some(Variant {
            name: "treatment".to_string(),
            enabled: true,
            payload: Some(VariantPayload {
                ty: FeatureFlagPayloadType::String,
                value: "hello".to_string(),
            }),
        })
    );
}

#[tokio::test]
async fn test_feature_flags_variant_disabled_but_feature_enabled() {
    let ctx = TestContext::new().await;

    let mock_response = GetUnleashFeaturesResponse {
        toggles: vec![UnleashToggle {
            name: "NoTreatment".to_string(),
            enabled: true,
            impression_data: false,
            variant: UnleashToggleVariant {
                name: "disabled".to_string(),
                enabled: false,
                feature_enabled: true,
                payload: None,
            },
        }],
    };

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
        .named("Variant disabled, feature on")
        .mount(ctx.mock_server())
        .await;

    let feature_flags = ctx.context().feature_flags();
    feature_flags.refresh().await.unwrap();

    assert_eq!(feature_flags.get("NoTreatment").await.unwrap(), Some(true));
    let variant = feature_flags
        .get_feature_flag_variant("NoTreatment")
        .await
        .unwrap();
    assert_eq!(
        variant,
        Some(Variant {
            name: "disabled".to_string(),
            enabled: false,
            payload: None,
        })
    );
}
