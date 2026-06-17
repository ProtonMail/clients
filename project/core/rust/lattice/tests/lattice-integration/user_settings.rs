use lattice::{
    LtApiResponseError, LtApiResponseErrorInfo,
    core::user_settings::{
        LtCoreGetSettingsReq, LtCoreGetSettingsRes, LtCorePutDeviceRecoveryPreferenceReq,
        LtCorePutDeviceRecoveryPreferenceRes,
    },
    details::AccessTokenWithInsufficientScopeErrorDetails,
};
use lattice_quark::{
    LtQuarkJSONRes,
    user::{
        LtQuarkUserStatus,
        user_create::{LtQuarkUserCreate, LtQuarkUserCreateRes},
    },
};

use crate::common::{generate_muon_session, login_muon_session, random_password, random_username};

#[tokio::test]
async fn test_get_user_settings_requires_auth() {
    let session = generate_muon_session().await;
    let res = session.send_lt(LtCoreGetSettingsReq).await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full"]
    );
}

#[tokio::test]
async fn test_get_user_settings() {
    let session = generate_muon_session().await;
    let username = random_username();
    let password = random_password();

    let res = session
        .send_quark(LtQuarkUserCreate {
            name: username.clone(),
            password: password.clone(),
            ..Default::default()
        })
        .await;
    assert_api_ok!(
        res,
        LtQuarkJSONRes(LtQuarkUserCreateRes {
            status: LtQuarkUserStatus::Active,
            ..
        })
    );

    let (session, _) = login_muon_session(session, &username, &password)
        .await
        .unwrap();
    let res = session.send_lt(LtCoreGetSettingsReq).await;
    assert_api_ok!(res, LtCoreGetSettingsRes { .. });
}

#[tokio::test]
async fn test_put_device_recovery_requires_auth() {
    let session = generate_muon_session().await;
    let res = session
        .send_lt(LtCorePutDeviceRecoveryPreferenceReq {
            device_recovery: true,
        })
        .await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full"]
    );
}

#[tokio::test]
async fn test_put_device_recovery_enable_disable() {
    let session = generate_muon_session().await;
    let username = random_username();
    let password = random_password();

    let res = session
        .send_quark(LtQuarkUserCreate {
            name: username.clone(),
            password: password.clone(),
            ..Default::default()
        })
        .await;
    assert_api_ok!(
        res,
        LtQuarkJSONRes(LtQuarkUserCreateRes {
            status: LtQuarkUserStatus::Active,
            ..
        })
    );

    let (session, _) = login_muon_session(session, &username, &password)
        .await
        .unwrap();

    let res = session
        .send_lt(LtCorePutDeviceRecoveryPreferenceReq {
            device_recovery: true,
        })
        .await;
    assert_api_ok!(res, LtCorePutDeviceRecoveryPreferenceRes {
        user_settings
    } if user_settings.device_recovery == Some(true));

    let res = session
        .send_lt(LtCorePutDeviceRecoveryPreferenceReq {
            device_recovery: false,
        })
        .await;
    assert_api_ok!(res, LtCorePutDeviceRecoveryPreferenceRes {
        user_settings
    } if user_settings.device_recovery == Some(false));
}
