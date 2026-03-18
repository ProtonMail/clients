mod common;

use lattice::{
    LtApiResponseError, LtApiResponseErrorInfo,
    auth::devices::{
        get_auth_devices::{LtAuthGetDevicesReq, LtAuthGetDevicesRes},
        post_auth_devices::LtAuthPostDevicesReq,
        post_auth_devices_associate::LtAuthPostDevicesAssociateReq,
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
async fn test_post_auth_devices() {
    let session = generate_muon_session().await;
    let res = session
        .send_lt(LtAuthPostDevicesReq {
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
        .send_lt(LtAuthPostDevicesReq {
            name: "Fairphone 4".to_string(),
            activation_token: None,
        })
        .await;
    assert_api_err!(&res, LtApiResponseError::InvalidPayload(_));
}
