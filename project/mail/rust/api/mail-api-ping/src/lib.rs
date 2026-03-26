use mail_api_shared::ApiServiceResult;
use mail_muon::common::{RetryPolicy, Sender};
use mail_muon::http::HttpReqExt;
use mail_muon::{GET, ProtonRequest, ProtonResponse};
use std::future::Future;
use std::time::Duration;

const CORE_V4: &str = "/core/v4";

#[allow(async_fn_in_trait)]
pub trait PingApi {
    fn get_tests_ping(
        &self,
        timeout: Option<Duration>,
        retry: Option<RetryPolicy>,
    ) -> impl Future<Output = ApiServiceResult<()>> + Send;
}

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> PingApi for This {
    async fn get_tests_ping(
        &self,
        timeout: Option<Duration>,
        retry: Option<RetryPolicy>,
    ) -> ApiServiceResult<()> {
        let mut req = GET!("{CORE_V4}/tests/ping");
        if let Some(t) = timeout {
            req = req.allowed_time(t);
        }
        if let Some(r) = retry {
            req = req.retry_policy(r);
        }
        req.send_with(self).await?.ok()?;
        Ok(())
    }
}
