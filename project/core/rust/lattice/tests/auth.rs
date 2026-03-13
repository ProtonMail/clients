mod common;

use lattice::{
    LtApiResponseError, LtApiResponseErrorInfo,
    auth::{
        LtAuthTwoFactorMethod,
        post_auth_2fa::{LtAuthPost2fa, LtAuthPost2faRes, LtAuthTwoFactorProof},
    },
    core::get_settings::{LtCoreGetSettingsReq, LtCoreGetSettingsRes},
    details::{
        AccessTokenWithInsufficientScopeErrorDetails, LoginFailedErrorDetails, LoginFailedReason,
    },
};
use muon::auth::scope;
use totp_rs::{Algorithm, Secret, TOTP};

use crate::common::{SessionExt, generate_muon_session, login_muon_session};

#[tokio::test]
async fn test_auth() {
    let session = generate_muon_session().await;
    let res = session.send_lt(LtCoreGetSettingsReq).await;
    assert_api_err!(&res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo {
            details: AccessTokenWithInsufficientScopeErrorDetails { missing_scopes },
            ..
        })
        if missing_scopes == &["full"]
    );
    let session_clone = async || session.client().get_session(()).await.unwrap();
    let session = login_muon_session(session_clone().await, "plus", "plus1").await;
    assert_api_err!(
        session,
        LtApiResponseError::LoginFailed(LtApiResponseErrorInfo {
            details: LoginFailedErrorDetails {
                login_failed_reason: LoginFailedReason::WrongPassword
            },
            ..
        })
    );
    let session = login_muon_session(session_clone().await, "plus_____1", "plus").await;
    assert_api_err!(
        session,
        LtApiResponseError::LoginFailed(LtApiResponseErrorInfo {
            details: LoginFailedErrorDetails {
                login_failed_reason: LoginFailedReason::WrongPassword
            },
            ..
        })
    );

    const USER_2FA: &str = "twofa";
    const PASS_2FA: &str = "a";
    const TOTP_2FA: &str = "4R5YJICSS6N72KNN3YRTEGLJCEKIMSKJ";
    let (session, tfa) = login_muon_session(session_clone().await, USER_2FA, PASS_2FA)
        .await
        .unwrap();
    assert!(tfa.is_some(), "{tfa:?} is expected to be Some");
    let tfa = tfa.unwrap();
    assert!(
        tfa.enabled.contains(LtAuthTwoFactorMethod::TOTP),
        "TFA {tfa:?} should contain TOTP"
    );

    let res = session
        .send_lt(LtAuthPost2fa {
            tfa_proof: LtAuthTwoFactorProof::Totp(add_one(next_totp(TOTP_2FA)).into()),
        })
        .await;
    assert_api_err!(
        res,
        LtApiResponseError::LoginFailed(LtApiResponseErrorInfo {
            details: LoginFailedErrorDetails {
                login_failed_reason: LoginFailedReason::WrongPassword
            },
            ..
        })
    );

    let res = session
        .send_lt(LtAuthPost2fa {
            tfa_proof: LtAuthTwoFactorProof::Totp(next_totp(TOTP_2FA).into()),
        })
        .await;
    assert_api_ok!(res, LtAuthPost2faRes { scopes } if scopes.contains(&scope::FULL.to_string()));

    let res = session.send_lt(LtCoreGetSettingsReq).await;
    assert_api_ok!(res, LtCoreGetSettingsRes { .. });
}

// "000000" -> "000001"; "999999" -> "000000"
fn add_one(s: impl AsRef<str>) -> String {
    let parsed = s.as_ref().parse::<u64>().unwrap();
    let next = (parsed + 1) % 1000000;
    format!("{:06}", next)
}

fn next_totp(secret: &str) -> String {
    const DIGITS: usize = 6;
    const SKEW: u8 = 1;
    const STEP: u64 = 30;
    const SHA1: Algorithm = Algorithm::SHA1;

    let secret = Secret::Encoded(secret.to_owned()).to_bytes().unwrap();
    let totp = &TOTP::new(SHA1, DIGITS, SKEW, STEP, secret).unwrap();
    totp.generate_current().unwrap()
}
