use anyhow::Result;
use mail_muon::GET;
use mail_muon::common::ConstProxy;
use mail_muon::test::proxy;
use mail_muon::test::server::{HTTPS, Server};
use mail_muon::util::ProtonRequestExt;
use std::sync::Arc;

#[mail_muon::test]
#[cfg_attr(ci, ignore = "local proxy not supported in CI")]
async fn test_ping_proxy_http(s: Arc<Server>) -> Result<()> {
    let proxy = ConstProxy::new(proxy::url()?.try_into()?);
    let client = s.builder().proxy(proxy).build()?;

    GET!("/tests/ping").send_with(&client).await?.ok()?;

    Ok(())
}

#[mail_muon::test(scheme(HTTPS))]
#[cfg_attr(ci, ignore = "local proxy not supported in CI")]
async fn test_ping_proxy_https(s: Arc<Server>) -> Result<()> {
    let proxy = ConstProxy::new(proxy::url()?.try_into()?);
    let client = s.builder().proxy(proxy).build()?;

    GET!("/tests/ping").send_with(&client).await?.ok()?;

    Ok(())
}
