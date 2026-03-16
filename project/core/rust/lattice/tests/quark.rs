mod common;

use lattice::{
    LtApiResponseError, LtApiResponseErrorInfo,
    core::get_settings::{LtCoreGetSettingsReq, LtCoreGetSettingsRes},
    details::{
        AccessTokenWithInsufficientScopeErrorDetails, LoginFailedErrorDetails, LoginFailedReason,
    },
    quark::{
        LtQuarkJSONRes,
        user::{
            LtQuarkUserStatus,
            user_create::{LtQuarkUserCreate, LtQuarkUserCreateRes},
        },
    },
};

use crate::common::{SessionExt, generate_muon_session, login_muon_session, random_string};

#[tokio::test]
async fn test_auth() {
    let session = generate_muon_session().await;

    let username = random_string(14);
    let password = random_string(34);

    let res = session
        .send_quark(LtQuarkUserCreate {
            name: username.to_string(),
            password: password.to_string(),
            mailbox_pass: Some("testtest1234".to_string()),
            ..Default::default()
        })
        .await;

    println!("{:?}", res);

    assert_api_ok!(res, LtQuarkJSONRes(LtQuarkUserCreateRes { status: LtQuarkUserStatus::Active, name: name_, password: password_, .. }) if name_ == &username && password_ == &password);

    let res = session.send_lt(LtCoreGetSettingsReq).await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full"]
    );
    let session_clone = async || session.client().get_session(()).await.unwrap();
    let session =
        login_muon_session(session_clone().await, &username, &format!("{password}1")).await;
    assert_api_err!(
        session,
        LtApiResponseError::LoginFailed(LtApiResponseErrorInfo {
            details: LoginFailedErrorDetails {
                login_failed_reason: LoginFailedReason::WrongPassword
            },
            ..
        })
    );

    let (session, tfa) = login_muon_session(session_clone().await, &username, &password)
        .await
        .unwrap();
    assert!(tfa.is_none(), "{tfa:?} is expected to be None");

    let res = session.send_lt(LtCoreGetSettingsReq).await;
    assert_api_ok!(res, LtCoreGetSettingsRes { .. });
}
