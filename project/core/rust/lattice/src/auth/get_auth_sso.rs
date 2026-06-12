//! SSO redirect entrypoint (`GET /auth/v4/sso/{token}`).
//!
//! On success the API returns an **HTML** page with a meta refresh to the IdP URL, not a SlimAPI JSON
//! envelope. [`LtAuthGetSsoRes`] therefore implements [`crate::LtResponseBody`] directly (HTML
//! parsing) instead of using [`crate::LtSlimAPIJSON`].

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;

use crate::{
    LatticeError, LtContract, LtRequestQueryParams, LtResponseBody, LtSlimAPIJSON, Sensitive,
    UnauthReq,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LtAuthGetSsoReq {
    /// Token received as SSOChallengeToken from POST /auth/info
    pub token: String,

    /// Optional final redirect base URL
    /// This URL is used by the IdP to redirect back after authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_redirect_base_url: Option<String>,
}

/// Parsed IdP redirect URL extracted from the HTML meta refresh (not JSON).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtAuthGetSsoRes {
    pub url: String,
}

pub struct LtAuthGetSsoQuery<'a> {
    pub final_redirect_base_url: &'a str,
}

impl LtRequestQueryParams for LtAuthGetSsoQuery<'_> {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        Ok(HashMap::from([(
            "FinalRedirectBaseUrl".into(),
            Sensitive::new(self.final_redirect_base_url.to_owned()),
        )]))
    }
}

impl LtResponseBody for LtAuthGetSsoRes {
    fn from_body(body: &[u8]) -> Result<Self, LatticeError> {
        let body = String::from_utf8(body.to_vec()).map_err(|e| {
            LatticeError::UnexpectedResponse(format!("Failed to parse HTML as UTF-8: {:?}", e))
        })?;
        // Uses find to avoid regex dependencies
        const HTML_START: &str = "<meta http-equiv=\"refresh\" content=\"0;url='";
        const HTML_END: &str = "'";
        // Send an invalid Body error if the HTML is not found
        let start = body.find(HTML_START).ok_or_else(|| {
            LatticeError::UnexpectedResponse(format!("No HTML start found in body: {}", body))
        })?;
        let html = &body[(start + HTML_START.len())..];
        let end = html.find(HTML_END).ok_or_else(|| {
            LatticeError::UnexpectedResponse(format!("No HTML end found in body: {}", html))
        })?;
        let url = &html[..end];

        Ok(LtAuthGetSsoRes {
            url: url.replace("&amp;", "&"),
        })
    }
}

impl LtContract for LtAuthGetSsoReq {
    /// Not [`LtSlimAPIJSON`] — success body is HTML; see module docs.
    type Response = LtAuthGetSsoRes;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtAuthGetSsoQuery<'q>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/auth/v4/sso/{}", self.token)))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        self.final_redirect_base_url
            .as_deref()
            .map(|final_redirect_base_url| LtAuthGetSsoQuery {
                final_redirect_base_url,
            })
    }
}

impl UnauthReq for LtAuthGetSsoReq {}
