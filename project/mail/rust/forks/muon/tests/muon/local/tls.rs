use anyhow::Result;
use mail_muon::GET;
use mail_muon::test::server::{HTTPS, Server};
use mail_muon::tls::{RustlsTls, TokioTls};
use std::sync::Arc;

#[mail_muon::test(scheme(HTTPS))]
async fn test_tls_rustls(s: Arc<Server>) -> Result<()> {
    let c = s.builder().tls(RustlsTls).build()?;

    c.send(GET!("/tests/ping")).await?.ok()?;

    Ok(())
}

#[mail_muon::test(scheme(HTTPS))]
#[ignore = "self-signed certificate"]
async fn test_tls_tokio(s: Arc<Server>) -> Result<()> {
    let c = s.builder().tls(TokioTls).build()?;

    c.send(GET!("/tests/ping")).await?.ok()?;

    Ok(())
}
