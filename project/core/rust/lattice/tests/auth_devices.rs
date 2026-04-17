mod common;

use lattice::{
    LtApiResponseError, LtApiResponseErrorInfo, Sensitive,
    auth::devices::{
        delete_auth_devices::LtAuthDeleteDevicesReq,
        get_auth_devices::{LtAuthGetDevicesReq, LtAuthGetDevicesRes},
        post_auth_devices_associate::LtAuthPostDevicesAssociateReq,
        post_auth_devices_create::LtAuthPostDevicesCreateReq,
        post_auth_devices_device_id::LtAuthPostDevicesDeviceIDReq,
        put_auth_devices_device_id_admin::LtAuthPutDevicesDeviceIDAdminReq,
    },
    details::AccessTokenWithInsufficientScopeErrorDetails,
};

use crate::common::{generate_muon_session, login_muon_session};

#[tokio::test]
async fn test_get_auth_devices() {
    let session = generate_muon_session().await;
    let res = session.send_lt(LtAuthGetDevicesReq).await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full"]
    );
    let (session, tfa) = login_muon_session(session, "plus", "plus").await.unwrap();
    assert!(tfa.is_none(), "{tfa:?} is expected to be None");
    let res = session.send_lt(LtAuthGetDevicesReq).await;
    assert_api_ok!(res, LtAuthGetDevicesRes { auth_devices } if auth_devices.is_empty());
}

#[tokio::test]
async fn test_post_auth_devices_associate() {
    let session = generate_muon_session().await;
    let res = session
        .send_lt(LtAuthPostDevicesAssociateReq {
            device_id: "1234567890".to_string(),
            device_token: "1234567890".to_string(),
        })
        .await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full"]
    );
    let (session, tfa) = login_muon_session(session, "plus", "plus").await.unwrap();
    assert!(tfa.is_none(), "{tfa:?} is expected to be None");
    let res = session
        .send_lt(LtAuthPostDevicesAssociateReq {
            device_id: "1234567890".to_string(),
            device_token: "1234567890".to_string(),
        })
        .await;
    assert_api_err!(&res, LtApiResponseError::InvalidDeviceID(_));
}

#[tokio::test]
async fn test_post_auth_devices_create() {
    let session = generate_muon_session().await;
    let res = session
        .send_lt(LtAuthPostDevicesCreateReq {
            name: "Fairphone 4".to_string(),
            activation_token: None,
        })
        .await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full", "locked"]
    );
    let (session, tfa) = login_muon_session(session, "plus", "plus").await.unwrap();
    assert!(tfa.is_none(), "{tfa:?} is expected to be None");
    let res = session
        .send_lt(LtAuthPostDevicesCreateReq {
            name: "Fairphone 4".to_string(),
            activation_token: None,
        })
        .await;
    assert_api_err!(&res, LtApiResponseError::InvalidPayload(_));
}

#[tokio::test]
async fn test_delete_auth_devices() {
    let session = generate_muon_session().await;
    let res = session
        .send_lt(LtAuthDeleteDevicesReq::DeviceID("1234567890".to_string()))
        .await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full"]
    );
    let (session, tfa) = login_muon_session(session, "plus", "plus").await.unwrap();
    assert!(tfa.is_none(), "{tfa:?} is expected to be None");
    let res = session
        .send_lt(LtAuthDeleteDevicesReq::DeviceID("1234567890".to_string()))
        .await;
    assert_api_err!(&res, LtApiResponseError::InvalidDeviceID(_));
    let res = session.send_lt(LtAuthDeleteDevicesReq::All).await;
    assert_api_ok!(res, _);
}

#[tokio::test]
async fn test_post_auth_devices_device_id() {
    let session = generate_muon_session().await;
    let res = session
        .send_lt(LtAuthPostDevicesDeviceIDReq {
            device_id: "1234567890".to_string(),
            encrypted_secret: Sensitive::new("secret".to_string()),
        })
        .await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full"]
    );
    let (session, tfa) = login_muon_session(session, "plus", "plus").await.unwrap();
    assert!(tfa.is_none(), "{tfa:?} is expected to be None");
    let res = session
        .send_lt(LtAuthPostDevicesDeviceIDReq {
            device_id: "1234567890".to_string(),
            encrypted_secret: Sensitive::new("secret".to_string()),
        })
        .await;
    assert_api_err!(&res, LtApiResponseError::InvalidDeviceID(_));
}

#[tokio::test]
async fn test_put_auth_devices_device_id_admin() {
    let session = generate_muon_session().await;
    let res = session
        .send_lt(LtAuthPutDevicesDeviceIDAdminReq {
            device_id: "1234567890".to_string(),
        })
        .await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full"]
    );
    let (session, tfa) = login_muon_session(session, "plus", "plus").await.unwrap();
    assert!(tfa.is_none(), "{tfa:?} is expected to be None");
    let res = session
        .send_lt(LtAuthPutDevicesDeviceIDAdminReq {
            device_id: "1234567890".to_string(),
        })
        .await;
    assert_api_err!(&res, LtApiResponseError::InvalidDeviceID(_));
}
