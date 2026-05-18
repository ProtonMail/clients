use anyhow::Result;
use mail_muon::GET;
use mail_muon::rt::{AsyncSpawner, PollWith};
use mail_muon::test::server::Server;
use std::sync::Arc;

#[mail_muon::test]
async fn test_runtime_dispatcher(s: Arc<Server>) -> Result<()> {
    // Create a dispatcher and its driver.
    let (dispatcher, driver) = mail_muon::rt::dispatcher();

    // Spawn the driver onto the runtime.
    AsyncSpawner::default().spawn(Box::pin(driver));

    // Create a client using the dispatcher.
    let c = s.builder().spawner(dispatcher).build()?;

    // This future will be executed by the dispatcher.
    c.send(GET!("/tests/ping")).await?.ok()?;

    Ok(())
}

#[mail_muon::test]
async fn test_runtime_dispatcher_poll_with(s: Arc<Server>) -> Result<()> {
    // Create a dispatcher and its driver.
    // We don't spawn the driver onto the runtime here.
    let (dispatcher, driver) = mail_muon::rt::dispatcher();

    // Create a client using the dispatcher.
    let c = s.builder().spawner(dispatcher).build()?;

    // This future will be executed by the current thread,
    // which will also drive the dispatcher.
    c.send(GET!("/tests/ping")).poll_with(&driver).await?.ok()?;

    Ok(())
}

#[mail_muon::test]
#[cfg(feature = "rt-tokio")]
async fn test_runtime_tokio(s: Arc<Server>) -> Result<()> {
    use mail_muon::rt::{TokioDialer, TokioResolver, TokioSpawner};

    // Create a client using the tokio runtime components.
    let c = s
        .builder()
        .resolver(TokioResolver)
        .dialer(TokioDialer)
        .spawner(TokioSpawner)
        .build()?;

    c.send(GET!("/tests/ping")).await?.ok()?;

    Ok(())
}
