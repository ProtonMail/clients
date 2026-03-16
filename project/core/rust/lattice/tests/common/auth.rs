use muon::auth::{Auth, Tokens};
use proton_crypto_account::proton_crypto::{
    CryptoError, new_srp_provider,
    srp::{ClientProof, SRPProvider},
};
use rand::{
    Rng,
    distr::{Alphabetic, Alphanumeric, SampleString, Uniform},
};
use serde_json::{Value, json};

use lattice::{
    LatticeError, Sensitive,
    auth::{
        LtAuthApiSession, LtAuthPasswordMode, LtAuthSrpChallenge, LtAuthTwoFactorOptions,
        post_auth::{LtAuthPostReq, LtAuthPostRes},
        post_auth_2fa::LtAuthSrpProof,
        post_auth_info::{LtAuthPostInfoReq, LtAuthPostInfoRes},
    },
};

use crate::common::{Session, SessionExt};

pub fn random_username() -> String {
    random_string(14)
}

pub fn random_password() -> String {
    random_string(34)
}

pub fn random_totp_secret() -> String {
    const BASE32_NOPAD: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut rng = rand::rng();
    let dist = Uniform::new(0, BASE32_NOPAD.len()).unwrap();
    let mut secret = String::with_capacity(32);
    for _ in 0..32 {
        secret.push(BASE32_NOPAD.chars().nth(rng.sample(dist)).unwrap());
    }
    secret.to_uppercase()
}

pub fn random_string(length: usize) -> String {
    Alphabetic.sample_string(&mut rand::rng(), length)
}

async fn login_get_proofs(
    username: &str,
    session: &Session,
) -> Result<LtAuthPostInfoRes, LatticeError> {
    session
        .send_lt(LtAuthPostInfoReq {
            username: Some(username.to_string()),
        })
        .await
}

fn gen_proofs(
    username: &str,
    password: &str,
    srp_challenge: &LtAuthSrpChallenge,
) -> Result<ClientProof, CryptoError> {
    new_srp_provider().generate_client_proof(
        username,
        password,
        srp_challenge.version,
        &srp_challenge.salt,
        &srp_challenge.modulus,
        &srp_challenge.server_ephemeral,
    )
}

pub async fn srp_handshake(
    session: &Session,
    username: &str,
    password: &str,
    challenge_info: Option<Value>,
) -> Result<
    (
        LtAuthApiSession,
        LtAuthPasswordMode,
        Option<LtAuthTwoFactorOptions>,
    ),
    LatticeError,
> {
    // Note: Username received in the response of auth/v4/info is necessary for auth versions v0 and v1.
    // Don't use this for anything other than passing it to the library that generates the client proof.
    let (username_old_auth, srp_challenge) = match login_get_proofs(username, session).await? {
        LtAuthPostInfoRes::SrpChallenge {
            username,
            srp_challenge,
            ..
        } => (username, srp_challenge),
        LtAuthPostInfoRes::SsoChallenge { .. } => unimplemented!("SSO is not yet implemented"),
    };

    let username_to_use = username_old_auth.as_deref().unwrap_or(username);

    let proof = gen_proofs(username_to_use, password, &srp_challenge).unwrap();

    let LtAuthPostRes {
        session,
        server_proof,
        password_mode,
        tfa,
    } = session
        .send_lt(LtAuthPostReq {
            username: username.to_string(),
            srp_proof: LtAuthSrpProof {
                srp_session: srp_challenge.session.clone(),
                client_ephemeral: Sensitive::new(proof.ephemeral.clone()),
                client_proof: Sensitive::new(proof.proof.clone()),
            },
            tfa_proof: None,
            payload: challenge_info,
        })
        .await?;

    proof
        .compare_server_proof(&server_proof)
        .then_some(())
        .unwrap();

    Ok((session, password_mode, tfa))
}

fn valid_fingerprint() -> Value {
    json!({
        "mail-android-99.9.40.0-challenge":{
            "appLang":"en",
            "deviceName":"TestDevice",
            "frame":{
                "name":"username"
            },
            "isDarkmodeOn":false,
            "isJailbreak":false,
            "keyboards":[

            ],
            "preferredContentSize":"2.0",
            "regionCode":"CH",
            "storageCapacity":"63.8",
            "timezone":"Europe/Zurich",
            "timezoneOffset":"0",
            "v":"2.0.0"
        }
    })
}

pub async fn login_muon_session(
    session: Session,
    username: &str,
    password: &str,
) -> Result<(Session, Option<LtAuthTwoFactorOptions>), LatticeError> {
    let (api_session, _password_mode, tfa) =
        srp_handshake(&session, username, password, Some(valid_fingerprint())).await?;
    let client = session.client().clone();
    let _ = session.remove_auth().await.unwrap();

    let credentials = Auth::internal(
        api_session.user_id,
        api_session.id,
        Tokens::access(
            api_session.access_token.into_inner(),
            api_session.refresh_token.into_inner(),
            api_session.scopes,
        ),
    );
    Ok((
        client
            .new_session_with_credentials((), credentials.try_into().unwrap())
            .await
            .unwrap(),
        tfa,
    ))
}
