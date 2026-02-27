use anyhow::Result;
use mail_muon::test::server::Server;
use mail_muon::GET;
use std::sync::Arc;

#[mail_muon::test]
async fn test_dropout(s: Arc<Server>) -> Result<()> {
    let c = s.client();

    // Send a request to establish a connection.
    c.send(GET!("/tests/ping")).await?.ok()?;

    // Forcibly close all connections server-side.
    // TODO: How to do this with axum?

    // Send a request to ensure the client reconnects.
    c.send(GET!("/tests/ping")).await?.ok()?;

    Ok(())
}
