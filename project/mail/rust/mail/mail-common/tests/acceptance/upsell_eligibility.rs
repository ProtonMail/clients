use mail_common::test_utils::init::Params as TestParams;
use mail_common::test_utils::test_context::MailTestContext;
use mail_common::{FF_UPSELL_EXPERIMENT, MailUserContext, UpsellEligibilityService};
use mail_core_api::services::proton::{
    GetLegacyFeaturesResponse, GetUnleashFeaturesResponse, UnleashToggle, UnleashTogglePayload,
    UnleashTogglePayloadType, UnleashToggleVariant, User as ApiUser,
};
use mail_core_common::datatypes::{UpsellEligibility, UpsellType};
use mail_core_common::models::{ModelExtension, PaidSubscription, Role, User};
use mail_core_common::test_utils::users::DEFAULT_USER;
use mail_stash::orm::Model;
use mail_stash::stash::{StashError, WriteTx};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

const USER: fn() -> ApiUser = || ApiUser {
    subscribed: 0,
    ..DEFAULT_USER()
};

#[tokio::test]
async fn mail_plus_upsell_when_flag_absent() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    setup_feature_flags(&ctx, None).await;

    let user_ctx = ctx.mail_user_context().await;
    let service = user_ctx.get_service::<UpsellEligibilityService>();
    let eligibility = service.upsell_eligibility().await.unwrap();

    assert_eq!(
        eligibility,
        UpsellEligibility::Eligible(UpsellType::MailPlus)
    );
}

#[tokio::test]
async fn mail_plus_upsell_when_payload_says_mailplus() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    setup_feature_flags(
        &ctx,
        Some(json_variant("MailPlus_USA", r#"{"upsell":"MailPlus"}"#)),
    )
    .await;

    let user_ctx = ctx.mail_user_context().await;
    let service = user_ctx.get_service::<UpsellEligibilityService>();
    let eligibility = service.upsell_eligibility().await.unwrap();

    assert_eq!(
        eligibility,
        UpsellEligibility::Eligible(UpsellType::MailPlus)
    );
}

#[tokio::test]
async fn unlimited_upsell_when_payload_says_unlimited() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    setup_feature_flags(
        &ctx,
        Some(json_variant(
            "Unlimited_Nordics",
            r#"{"upsell":"Unlimited"}"#,
        )),
    )
    .await;

    let user_ctx = ctx.mail_user_context().await;
    user_ctx
        .user_context()
        .feature_flags()
        .refresh()
        .await
        .expect("Fresh feature flags");

    let service = user_ctx.get_service::<UpsellEligibilityService>();
    let eligibility = service.upsell_eligibility().await.unwrap();

    assert_eq!(
        eligibility,
        UpsellEligibility::Eligible(UpsellType::Unlimited)
    );
}

#[tokio::test]
async fn mail_plus_upsell_when_payload_is_malformed_json() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    setup_feature_flags(
        &ctx,
        Some(json_variant("Bogus", r#"{"upsell":"NotAPlan"}"#)),
    )
    .await;

    let user_ctx = ctx.mail_user_context().await;
    let service = user_ctx.get_service::<UpsellEligibilityService>();
    let eligibility = service.upsell_eligibility().await.unwrap();

    assert_eq!(
        eligibility,
        UpsellEligibility::Eligible(UpsellType::MailPlus)
    );
}

#[tokio::test]
async fn mail_plus_upsell_when_variant_disabled() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    setup_feature_flags(
        &ctx,
        Some(UnleashToggleVariant {
            name: "Unlimited_Nordics".to_string(),
            enabled: false,
            feature_enabled: true,
            payload: Some(UnleashTogglePayload {
                ty: UnleashTogglePayloadType::Json,
                value: r#"{"upsell":"Unlimited"}"#.to_string(),
            }),
        }),
    )
    .await;

    let user_ctx = ctx.mail_user_context().await;
    let service = user_ctx.get_service::<UpsellEligibilityService>();
    let eligibility = service.upsell_eligibility().await.unwrap();

    assert_eq!(
        eligibility,
        UpsellEligibility::Eligible(UpsellType::MailPlus)
    );
}

#[tokio::test]
async fn paid_user_not_eligible() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    setup_feature_flags(&ctx, None).await;

    let user_ctx = ctx.mail_user_context().await;
    user_ctx
        .user_context()
        .feature_flags()
        .refresh()
        .await
        .expect("Fresh feature flags");

    let user_stash = user_ctx.user_stash();
    let mut tether = user_stash.connection();
    tether
        .write_tx(async |tx| save_subscription(&user_ctx, PaidSubscription::MAIL, tx).await)
        .await
        .unwrap();

    let service = user_ctx.get_service::<UpsellEligibilityService>();
    let eligibility = service.upsell_eligibility().await.unwrap();
    assert_eq!(eligibility, UpsellEligibility::NotEligible);
}

// We do not show this upsell promotion for users that are not paying for mail but are still
// paying for other services.
#[tokio::test]
async fn paid_user_other_services_not_eligible() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    setup_feature_flags(&ctx, None).await;

    let user_ctx = ctx.mail_user_context().await;
    user_ctx
        .user_context()
        .feature_flags()
        .refresh()
        .await
        .expect("Fresh feature flags");

    let user_stash = user_ctx.user_stash();
    let mut tether = user_stash.connection();
    tether
        .write_tx(async |tx| save_subscription(&user_ctx, PaidSubscription::VPN, tx).await)
        .await
        .unwrap();

    let service = user_ctx.get_service::<UpsellEligibilityService>();
    let eligibility = service.upsell_eligibility().await.unwrap();
    assert_eq!(eligibility, UpsellEligibility::NotEligible);
}

#[tokio::test]
async fn member_role_not_eligible() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    setup_feature_flags(&ctx, None).await;

    let user_ctx = ctx.mail_user_context().await;
    user_ctx
        .user_context()
        .feature_flags()
        .refresh()
        .await
        .expect("Fresh feature flags");

    let user_stash = user_ctx.user_stash();
    let mut tether = user_stash.connection();
    tether
        .write_tx(async |tx| save_role(&user_ctx, Role::Member, tx).await)
        .await
        .unwrap();

    let service = user_ctx.get_service::<UpsellEligibilityService>();
    let eligibility = service.upsell_eligibility().await.unwrap();
    assert_eq!(eligibility, UpsellEligibility::NotEligible);
}

async fn save_subscription(
    ctx: &MailUserContext,
    subscription: PaidSubscription,
    tx: &WriteTx<'_>,
) -> Result<(), StashError> {
    let mut user = User::find_by_id(ctx.user_id().clone(), tx).await?.unwrap();
    user.subscribed = subscription;
    user.save(tx).await
}

async fn save_role(ctx: &MailUserContext, role: Role, tx: &WriteTx<'_>) -> Result<(), StashError> {
    let mut user = User::find_by_id(ctx.user_id().clone(), tx).await?.unwrap();
    user.role = role;
    user.save(tx).await
}

fn json_variant(name: &str, payload: &str) -> UnleashToggleVariant {
    UnleashToggleVariant {
        name: name.to_string(),
        enabled: true,
        feature_enabled: true,
        payload: Some(UnleashTogglePayload {
            ty: UnleashTogglePayloadType::Json,
            value: payload.to_string(),
        }),
    }
}

async fn setup_feature_flags(ctx: &MailTestContext, variant: Option<UnleashToggleVariant>) {
    let toggles = variant
        .map(|variant| {
            vec![UnleashToggle {
                name: FF_UPSELL_EXPERIMENT.to_string(),
                enabled: true,
                impression_data: false,
                variant,
            }]
        })
        .unwrap_or_default();

    let mock_response = GetUnleashFeaturesResponse { toggles };

    Mock::given(method("GET"))
        .and(path("/api/feature/v2/frontend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
        .named("Feature flags setup")
        .mount(ctx.mock_server())
        .await;

    Mock::given(method("GET"))
        .and(path("/api/core/v4/features"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(GetLegacyFeaturesResponse {
                total: 0,
                features: vec![],
            }),
        )
        .named("Empty Legacy fetch")
        .mount(ctx.mock_server())
        .await;
}
