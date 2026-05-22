// Lattice tests chain `Session` → `Muon2Transport` → `LtContract` over muon's `GenericContext`
// (connector, store, CookieStore). Muon 2.4+ deepens those nested generics; rustc's default
// `recursion_limit` (128) is exceeded when computing async fn layout (e.g. unprivatize helpers).
#![recursion_limit = "256"]

mod common;

use lattice::{
    LtApiResponseError, LtApiResponseErrorInfo,
    core::user_settings::{LtCoreGetSettingsReq, LtCoreGetSettingsRes},
    details::{
        AccessTokenWithInsufficientScopeErrorDetails, LoginFailedErrorDetails, LoginFailedReason,
    },
};
use lattice_quark::{
    LtQuarkJSONRes,
    user::{
        LtQuarkUserStatus,
        user_create::{LtQuarkUserCreate, LtQuarkUserCreateRes},
    },
};

use crate::common::{generate_muon_session, login_muon_session, random_string};

#[tokio::test]
async fn test_auth() {
    let session_init = generate_muon_session().await;

    let username = random_string(14);
    let password = random_string(34);

    let res = session_init
        .send_quark(LtQuarkUserCreate {
            name: username.to_string(),
            password: password.to_string(),
            mailbox_pass: Some("testtest1234".to_string()),
            ..Default::default()
        })
        .await;

    println!("{:?}", res);

    assert_api_ok!(res, LtQuarkJSONRes(LtQuarkUserCreateRes { status: LtQuarkUserStatus::Active, name: name_, password: password_, .. }) if name_ == &username && password_ == &password);

    let res = session_init.send_lt(LtCoreGetSettingsReq).await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full"]
    );
    let session = login_muon_session(
        session_init.clone().await,
        &username,
        &format!("{password}1"),
    )
    .await;
    assert_api_err!(
        session,
        LtApiResponseError::LoginFailed(LtApiResponseErrorInfo {
            details: LoginFailedErrorDetails {
                login_failed_reason: LoginFailedReason::WrongPassword
            },
            ..
        })
    );

    let (session, tfa) = login_muon_session(session_init.clone().await, &username, &password)
        .await
        .unwrap();
    assert!(tfa.is_none(), "{tfa:?} is expected to be None");

    let res = session.send_lt(LtCoreGetSettingsReq).await;
    assert_api_ok!(res, LtCoreGetSettingsRes { .. });
}
