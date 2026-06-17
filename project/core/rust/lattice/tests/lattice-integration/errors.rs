use lattice::{
    LatticeError, LtApiResponseError, LtApiResponseErrorInfo,
    auth::devices::get_auth_devices::LtAuthGetDevicesReq,
    details::AccessTokenWithInsufficientScopeErrorDetails,
};
use lattice_muon2::LtTransportError;

use crate::common::generate_muon_session;

#[tokio::test]
async fn test_errors() {
    let session = generate_muon_session().await;
    let res = session.send_lt(LtAuthGetDevicesReq).await;
    assert!(res.is_err(), "{res:?} is expected to be Err");
    let err = res.unwrap_err();
    let lattice_err = match err {
        LtTransportError::Lattice(e) => e,
        LtTransportError::Transport(t) => panic!("unexpected transport: {t:?}"),
    };
    assert!(
        matches!(lattice_err, LatticeError::ApiError(403, _)),
        "{lattice_err:?} is expected to be ApiError"
    );
    let api_error = lattice_err.as_api_error().unwrap();
    assert!(
        matches!(&api_error, LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo { details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes }, .. }) if missing_scopes == &["full"]),
        "{:?} is expected to be AccessTokenWithInsufficientScope",
        api_error
    );
    let api_error =
        if let LtApiResponseError::AccessTokenWithInsufficientScope(api_error) = api_error {
            api_error
        } else {
            panic!("api_error is expected to be AccessTokenWithInsufficientScope");
        };
    assert_eq!(
        api_error.error,
        "Access token does not have sufficient scope"
    );
    assert_eq!(
        api_error.metadata.exception,
        Some("Proton\\Http\\Exceptions\\ForbiddenException".to_string())
    );
    assert_eq!(
        api_error.metadata.message,
        Some("Access token does not have sufficient scope".to_string())
    );
    assert!(
        api_error.metadata.file.is_some(),
        "file is expected to be Some"
    );
    assert!(
        api_error.metadata.line.is_some(),
        "line is expected to be Some"
    );
    assert!(
        api_error.metadata.trace.is_some(),
        "trace is expected to be Some"
    );
    let trace = api_error.metadata.trace.as_ref().unwrap();
    assert!(!trace.is_empty(), "trace is expected to be non-empty");
}
