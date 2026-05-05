use std::sync::Arc;

use core_event_loop::v6::EventSubscriber;
use mail_core_api::{
    consts::General,
    service::ApiErrorInfo,
    services::proton::{AddressEventV6, CoreEventV6},
};
use mail_core_common::{
    event_loop::v6::{CoreEventCache, CoreEventSourceV6, CoreEventV6Subscriber},
    services::event_loop_service::EventManagerContext,
    test_utils::test_context::TestContext,
};
use mail_core_key_manager::AddressId;

#[tokio::test]
async fn missing_address_does_not_fail_event_poll() {
    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;

    let id = AddressId::from("Addr");

    ctx.mock_get_address_by_id(
        id.clone(),
        Err((
            422,
            ApiErrorInfo {
                code: General::NotExists as u32,
                error: None,
                details: None,
            },
        )),
    )
    .await;

    let event = CoreEventV6 {
        users: None,
        addresses: Some(vec![AddressEventV6 {
            id: id.clone(),
            action: mail_core_api::services::proton::Action::Update,
        }]),
        user_settings: None,
        refresh: false,
        has_more: false,
    };

    let subscriber = CoreEventV6Subscriber::from(Arc::downgrade(&user_ctx));
    let mut cache = CoreEventCache::default();

    <CoreEventV6Subscriber as EventSubscriber<EventManagerContext, CoreEventSourceV6>>::on_event(
        &subscriber,
        &user_ctx,
        &event,
        &mut cache,
    )
    .await
    .unwrap();
}
