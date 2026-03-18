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
    quark::{
        LtQuarkJSONRes,
        user::{
            LtQuarkUserStatus,
            user_create::{LtQuarkUserCreate, LtQuarkUserCreateRes},
        },
    },
};
use muon::auth::scope;
use totp_rs::{Algorithm, Secret, TOTP};

use crate::common::{
    generate_muon_session, login_muon_session, random_password, random_totp_secret, random_username,
};

#[tokio::test]
async fn test_auth() {
    let session_init = generate_muon_session().await;

    let username = random_username();
    let password = random_password();
    let totp_secret = random_totp_secret();

    let res = session_init
        .send_quark(LtQuarkUserCreate {
            name: username.to_string(),
            password: password.to_string(),
            totp_secret: Some(totp_secret.to_string()),
            ..Default::default()
        })
        .await;

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
    let session = login_muon_session(
        session_init.clone().await,
        &format!("{username}1"),
        &password,
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
    assert!(tfa.is_some(), "{tfa:?} is expected to be Some");
    let tfa = tfa.unwrap();
    assert!(
        tfa.enabled.contains(LtAuthTwoFactorMethod::TOTP),
        "TFA {tfa:?} should contain TOTP"
    );

    let res = session
        .send_lt(LtAuthPost2fa {
            tfa_proof: LtAuthTwoFactorProof::Totp(add_one(next_totp(&totp_secret)).into()),
        })
        .await;
    assert_api_err!(
        res,
        LtApiResponseError::LoginFailed(LtApiResponseErrorInfo {
            details: LoginFailedErrorDetails {
                login_failed_reason: LoginFailedReason::TotpWrong
            },
            ..
        })
    );

    let res = session
        .send_lt(LtAuthPost2fa {
            tfa_proof: LtAuthTwoFactorProof::Totp(next_totp(&totp_secret).into()),
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
