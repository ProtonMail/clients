use crate::atlas::new_builder;
use anyhow::Result;
use mail_muon::GET;
use mail_muon::tls::{RustlsTls, TokioTls};

#[tokio::test]
async fn test_tls_rustls() -> Result<()> {
    let c = new_builder().tls(RustlsTls).build()?;

    c.send(GET!("/tests/ping")).await?.ok()?;

    Ok(())
}

#[tokio::test]
async fn test_tls_tokio() -> Result<()> {
    let c = new_builder().tls(TokioTls).build()?;

    c.send(GET!("/tests/ping")).await?.ok()?;

    Ok(())
}
