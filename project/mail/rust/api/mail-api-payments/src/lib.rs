//! HTTP bridge for the Payment SDK.

use mail_api_session::session::Session;
use mail_api_shared::ApiServiceResult;
use mail_muon::http::HttpReqExt;
use mail_muon::{GET, POST};

/// HTTP response forwarded to the Payment SDK.
#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub async fn http_get(session: &Session, endpoint: String) -> ApiServiceResult<HttpResponse> {
    let res = GET!("{endpoint}").send_with(session).await?;
    Ok(HttpResponse {
        status: res.status().as_u16(),
        body: res.into_body(),
    })
}

pub async fn http_post(
    session: &Session,
    endpoint: String,
    body: Vec<u8>,
) -> ApiServiceResult<HttpResponse> {
    let res = POST!("{endpoint}").body(body).send_with(session).await?;
    Ok(HttpResponse {
        status: res.status().as_u16(),
        body: res.into_body(),
    })
}
