use lattice::{
    LatticeError, LtApiResponseError,
    core::user::get_users_available_external::LtCoreGetUsersAvailableExternalReq,
};
use lattice_muon2::LtTransportError;

use crate::common::{generate_muon_session, random_username};

/// Calling `GET /core/v4/users/availableExternal` from an unauthenticated session against
/// Atlas always trips `AbuseHumanVerificationFactory#verifyExternalSignup` in the
/// `UserController#externalAvailable` handler. We can't drive an actual HV flow from an
/// integration test, so we instead assert that this is exactly what we get.
///
/// That's still a meaningful contract:
///   - the request is dispatched at all (muon transport works for an `UnauthReq`),
///   - the path `/core/v4/users/availableExternal` and the `Name` query param reach the
///     right controller (otherwise the error would be 404 / a different validator),
///   - we successfully deserialize the response into `LtApiResponseError::HumanVerification`
///     (code 9001), confirming that `HumanVerificationErrorDetails` matches the wire shape.
#[tokio::test]
async fn test_available_external_unverified_caller_requires_hv() {
    let session = generate_muon_session().await;
    let res = session
        .send_lt(LtCoreGetUsersAvailableExternalReq {
            name: format!("{}@gmail.com", random_username()),
            payment_info_token: None,
        })
        .await;
    assert_api_err!(res, LtApiResponseError::HumanVerification(_));
}

/// An empty `Name` is rejected by the controller's input validation upstream of the HV
/// gate, surfacing as a 400 BadRequest with code 2000 ("Invalid input"). Pinning the
/// observed status locks in that we go through the `LatticeError::ApiError` path
/// (see `tests/errors.rs`) and that the controller is the one rejecting us.
#[tokio::test]
async fn test_available_external_invalid_name_is_rejected() {
    let session = generate_muon_session().await;
    let res = session
        .send_lt(LtCoreGetUsersAvailableExternalReq {
            name: String::new(),
            payment_info_token: None,
        })
        .await;
    let err = res.expect_err("empty name should be rejected");
    let lattice_err = match err {
        LtTransportError::Lattice(e) => e,
        LtTransportError::Transport(t) => panic!("unexpected transport: {t:?}"),
    };
    assert!(
        matches!(lattice_err, LatticeError::ApiError(400, _)),
        "{lattice_err:?} expected to be ApiError(400, _)",
    );
}
