use std::fmt::Display;

use crate::common_sso::{extract_field, get_token_from_html};

#[derive(Debug)]
pub struct SAMLForm {
    pub action: String,
    pub saml_response: String,
    pub relay_state: String,
}

#[derive(Debug)]
pub enum SAMLParsingError {
    MissingAction,
    MissingSAMLResponse,
    MissingRelayState,
    ReqwestError(reqwest::Error),
}

impl std::fmt::Display for SAMLParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SAMLParsingError::MissingAction => write!(f, "SAML form action not found in HTML"),
            SAMLParsingError::MissingSAMLResponse => {
                write!(f, "SAMLResponse field not found in HTML")
            }
            SAMLParsingError::MissingRelayState => write!(f, "RelayState field not found in HTML"),
            SAMLParsingError::ReqwestError(e) => write!(f, "Reqwest error: {:?}", e),
        }
    }
}

impl std::error::Error for SAMLParsingError {}

impl SAMLForm {
    pub async fn from_challenge_url(url: &str) -> Result<SAMLForm, SAMLParsingError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Failed to build client");

        let res = client
            .get(url)
            .send()
            .await
            .map_err(SAMLParsingError::ReqwestError)?;
        let body = res.text().await.map_err(SAMLParsingError::ReqwestError)?;

        SAMLForm::from_html(&body)
    }

    pub fn from_html(html: &str) -> Result<SAMLForm, SAMLParsingError> {
        let action = extract_field(
            html,
            r#"<form id="samlForm" action="([^"]+)" method="post""#,
        )
        .ok_or(SAMLParsingError::MissingAction)?;

        let saml_response = extract_field(
            html,
            r#"<input type="hidden" name="SAMLResponse" value="([^"]+)">"#,
        )
        .ok_or(SAMLParsingError::MissingSAMLResponse)?;

        let relay_state = extract_field(
            html,
            r#"<input type="hidden" name="RelayState" value="([^"]+)">"#,
        )
        .ok_or(SAMLParsingError::MissingRelayState)?;

        Ok(SAMLForm {
            action,
            saml_response,
            relay_state,
        })
    }

    pub async fn request_sso_token(&self) -> Result<String, SAMLPostError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let res = client
            .post(&self.action)
            .form(&[
                ("SAMLResponse", &self.saml_response),
                ("RelayState", &self.relay_state),
            ])
            .send()
            .await
            .map_err(SAMLPostError::ReqwestError)?;
        let body = res.text().await.map_err(SAMLPostError::ReqwestError)?;
        get_token_from_html(&body).ok_or(SAMLPostError::NoTokenFoundInHTML(body))
    }
}

#[derive(Debug)]
pub enum SAMLPostError {
    NoTokenFoundInHTML(String),
    ReqwestError(reqwest::Error),
}

impl std::error::Error for SAMLPostError {}

impl Display for SAMLPostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SAMLPostError::NoTokenFoundInHTML(body) => {
                write!(f, "No token found in HTML: {}", body)
            }
            SAMLPostError::ReqwestError(e) => write!(f, "Reqwest error: {:?}", e),
        }
    }
}
