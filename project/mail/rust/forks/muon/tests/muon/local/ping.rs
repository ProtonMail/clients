use anyhow::Result;
use mail_muon::test::server::{Server, HTTP, HTTPS};
use mail_muon::GET;
use std::sync::Arc;

#[mail_muon::test(scheme(HTTP))]
async fn test_ping_http(s: Arc<Server>) -> Result<()> {
    s.client().send(GET!("/tests/ping")).await?.ok()?;

    Ok(())
}

#[mail_muon::test(scheme(HTTPS))]
async fn test_ping_https(s: Arc<Server>) -> Result<()> {
    s.client().send(GET!("/tests/ping")).await?.ok()?;

    Ok(())
}
