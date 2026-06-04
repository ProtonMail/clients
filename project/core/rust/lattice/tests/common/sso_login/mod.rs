//! SAML challenge flow for SSO integration tests (IdP HTML form → SSO token → session).

use lattice::{
    LatticeError,
    auth::{
        get_auth_sso::LtAuthGetSsoReq,
        post_auth::LtAuthPostSsoReq,
        post_auth_info::{LtAuthPostInfoIntent, LtAuthPostInfoReq, LtAuthPostInfoRes},
    },
};
use std::sync::LazyLock;

use lattice_muon2::LtTransportError;
use regex::Regex;

use super::Session;
use super::sso_login::saml_form::SAMLForm;

pub mod saml_form;

static SSO_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"protonpass://[a-zA-Z0-9.-]+\.proton\.black/sso/login#token=([^&"'\s]+)"#)
        .expect("valid SSO token regex")
});

static SAML_FORM_ACTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<form id="samlForm" action="([^"]+)" method="post""#)
        .expect("valid SAML action regex")
});

static SAML_RESPONSE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<input type="hidden" name="SAMLResponse" value="([^"]+)">"#)
        .expect("valid SAMLResponse regex")
});

static SAML_RELAY_STATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<input type="hidden" name="RelayState" value="([^"]+)">"#)
        .expect("valid RelayState regex")
});

pub fn extract_field(html: &str, re: &Regex) -> Option<String> {
    re.captures(html)?.get(1).map(|m| m.as_str().to_string())
}

pub fn get_token_from_html(html: &str) -> Option<String> {
    extract_field(html, &SSO_TOKEN_RE)
}

pub async fn get_challenge_url_from_challenge_token(
    session: &Session,
    challenge_token: &str,
) -> Result<String, LtTransportError> {
    session
        .send_lt_generic(LtAuthGetSsoReq {
            token: challenge_token.to_string(),
            final_redirect_base_url: Some("protonpass://account.lagrange.proton.black".to_string()),
        })
        .await
        .map(|res| res.url)
}

pub async fn get_sso_challenge(
    session: &Session,
    username: &str,
) -> Result<String, LtTransportError> {
    let res = session
        .send_lt(LtAuthPostInfoReq {
            username: Some(username.to_string()),
            client_secret: None,
            intent: Some(LtAuthPostInfoIntent::Sso),
            is_testing: None,
            reauth_scope: None,
        })
        .await?;

    let sso_challenge_token = match res {
        LtAuthPostInfoRes::SsoChallenge {
            sso_challenge_token,
        } => sso_challenge_token,
        _ => {
            return Err(LtTransportError::from(LatticeError::UnexpectedStatusCode(
                400,
                "Expected SSO challenge response".as_bytes().to_vec(),
            )));
        }
    };

    Ok(sso_challenge_token)
}

pub async fn login_with_sso(
    session_init: Session,
    username: &str,
) -> Result<Session, LtTransportError> {
    let sso_challenge_token = get_sso_challenge(&session_init, username).await?;

    let url = get_challenge_url_from_challenge_token(&session_init, &sso_challenge_token)
        .await
        .map_err(|e| {
            LtTransportError::from(LatticeError::UnexpectedStatusCode(
                400,
                format!("Failed to get challenge URL: {:?}", e)
                    .as_bytes()
                    .to_vec(),
            ))
        })?;

    let form = SAMLForm::from_challenge_url(&url).await.map_err(|e| {
        LtTransportError::from(LatticeError::UnexpectedStatusCode(
            400,
            format!("Failed to parse SAML form from HTML: {:?}", e)
                .as_bytes()
                .to_vec(),
        ))
    })?;

    let token = form.request_sso_token().await.map_err(|e| {
        LtTransportError::from(LatticeError::UnexpectedStatusCode(
            400,
            format!("Failed to request SSO token: {:?}", e)
                .as_bytes()
                .to_vec(),
        ))
    })?;

    let api_session = session_init
        .send_lt(LtAuthPostSsoReq {
            sso_response_token: token,
        })
        .await?
        .session;

    Ok(session_init.swap_session(api_session).await)
}
