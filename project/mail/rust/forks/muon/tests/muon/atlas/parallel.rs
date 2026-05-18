use crate::atlas::{PASS, USER, new_builder};
use anyhow::{Ok, Result, bail};
use futures::future;
use mail_muon::client::flow::LoginFlow;
use mail_muon::{Client, GET};
use serde_json::Value;

#[tokio::test]
async fn test_parallel() -> Result<()> {
    let c = match new_builder()
        .build()
        .expect("client should build")
        .auth()
        .login(USER, PASS)
        .await
    {
        LoginFlow::Ok(c, _) => c,
        LoginFlow::TwoFactor(_, _) => bail!("unexpected TFA flow"),
        LoginFlow::Failed { .. } => bail!("unexpected failure"),
    };

    let t = (0..10)
        .map(|_| c.clone())
        .map(|c| send_loop(c, 10))
        .map(|f| tokio::spawn(f))
        .collect::<Vec<_>>();

    for res in future::join_all(t).await {
        res??;
    }

    Ok(())
}

async fn send_loop(c: Client, n: usize) -> Result<()> {
    for _ in 0..n {
        let req = GET!("/tests/ping").allowed_time(std::time::Duration::from_secs(60));
        let res = c.send(req).await?;
        let _: Value = res.ok()?.into_body_json()?;
    }

    Ok(())
}
