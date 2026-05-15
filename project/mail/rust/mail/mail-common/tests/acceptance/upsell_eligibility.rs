use mail_common::test_utils::init::Params as TestParams;
use mail_common::test_utils::test_context::MailTestContext;
use mail_common::{
    FF_UPSELL_UNLIMITED_CHILD, FF_UPSELL_UNLIMITED_PARENT, MailUserContext,
    UpsellEligibilityService,
};
use mail_core_api::services::proton::{
    GetLegacyFeaturesResponse, GetUnleashFeaturesResponse, UnleashToggle, UnleashToggleVariant,
    User as ApiUser,
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
async fn mail_plus_upsell_when_unlimited_flag_disabled() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    mount_feature_flag_mocks(&ctx, TestedFeatureFlags::default()).await;

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
        UpsellEligibility::Eligible(UpsellType::MailPlus)
    );
}

#[tokio::test]
async fn unlimited_upsell_when_unlimited_flag_enabled() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    mount_feature_flag_mocks(
        &ctx,
        TestedFeatureFlags {
            upsell_unlimited_parent: true,
            upsell_unlimited_child: true,
        },
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
async fn paid_user_not_eligible() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic().with_user(USER());
    ctx.setup_user(params).await;
    mount_feature_flag_mocks(&ctx, TestedFeatureFlags::default()).await;

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
    mount_feature_flag_mocks(&ctx, TestedFeatureFlags::default()).await;

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
    mount_feature_flag_mocks(&ctx, TestedFeatureFlags::default()).await;

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

fn test_unleash_variant() -> UnleashToggleVariant {
    UnleashToggleVariant {
        name: "enabled".to_string(),
        enabled: true,
        feature_enabled: true,
        payload: None,
    }
}

#[derive(Default)]
struct TestedFeatureFlags {
    upsell_unlimited_parent: bool,
    upsell_unlimited_child: bool,
}

async fn mount_feature_flag_mocks(ctx: &MailTestContext, flags: TestedFeatureFlags) {
    let mut toggles = vec![];

    if flags.upsell_unlimited_parent {
        toggles.push(UnleashToggle {
            name: FF_UPSELL_UNLIMITED_PARENT.to_string(),
            enabled: true,
            impression_data: false,
            variant: test_unleash_variant(),
        });
    }

    if flags.upsell_unlimited_child {
        toggles.push(UnleashToggle {
            name: FF_UPSELL_UNLIMITED_CHILD.to_string(),
            enabled: true,
            impression_data: false,
            variant: test_unleash_variant(),
        });
    }

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
