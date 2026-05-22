// Lattice tests chain `Session` → `Muon2Transport` → `LtContract` over muon's `GenericContext`
// (connector, store, CookieStore). Muon 2.4+ deepens those nested generics; rustc's default
// `recursion_limit` (128) is exceeded when computing async fn layout (e.g. unprivatize helpers).
#![recursion_limit = "256"]

mod common;

use lattice_quark::{
    LtQuarkJSONRes,
    payments::subscribed_user_seed::LtQuarkNewPaymentsSeedSubscribedUser,
    user::organization_create::{LtQuarkUserCreateOrganization, LtQuarkUserCreateOrganizationRes},
};

use crate::common::{generate_muon_session, random_password, random_username};

#[tokio::test]
async fn test_create_organization() {
    let session_init = generate_muon_session().await;

    // First, create a user that will become the admin
    let username = random_username();
    let password = random_password();

    let user_res = session_init
        .send_quark(LtQuarkNewPaymentsSeedSubscribedUser {
            username: username.to_string(),
            password: password.to_string(),
            plan: Some("visionary2022".to_string()),
            // gen_keys: Some(LtQuarkKeyType::Curve25519),
            ..Default::default()
        })
        .await
        .unwrap();

    // Extract user_id from the result
    let user_id = user_res.user_id;

    // Now create an organization with this user as admin
    let org_res = session_init
        .send_quark(LtQuarkUserCreateOrganization {
            user_id,
            password: password.to_string(),
            ..Default::default()
        })
        .await;

    assert_api_ok!(
        org_res,
        LtQuarkJSONRes(LtQuarkUserCreateOrganizationRes { .. })
    );
}
