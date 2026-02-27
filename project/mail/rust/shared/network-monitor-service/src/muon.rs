use crate::{ConnectionMonitor, RequestNetworkStatus};
use mail_muon::common::{BoxFut, Sender, SenderLayer, Timeout};
use mail_muon::{ProtonRequest, ProtonResponse};
use std::error::Error;

impl ConnectionMonitor {
    async fn on_send<S>(&self, inner: &S, req: ProtonRequest) -> mail_muon::Result<ProtonResponse>
    where
        S: Sender<ProtonRequest, ProtonResponse> + ?Sized,
    {
        let r = inner.send(req).await;
        self.inspect_result(&r);
        r
    }

    pub fn inspect_result(&self, result: &mail_muon::Result<ProtonResponse>) {
        match result {
            Ok(resp) => {
                self.on_recv_ok(resp);
            }

            Err(error) => {
                self.on_recv_err(error);
            }
        }
    }

    fn on_recv_err(&self, error: &mail_muon::Error) {
        use mail_muon::error::ErrorKind;

        match error.kind() {
            ErrorKind::Tls | ErrorKind::Resolve | ErrorKind::Dial => {
                self.update_request_status(RequestNetworkStatus::Offline);
            }
            ErrorKind::Send => {
                // We want to ignore mail_muon's built in max time limit from the network detection
                // logic since this can also be caused by long server response or slow network. This
                // in turn does not mean that there is no network.
                #[allow(clippy::redundant_closure_for_method_calls)] // false positive
                if !error.source().is_some_and(|s| s.is::<Timeout>()) {
                    self.update_request_status(RequestNetworkStatus::Offline);
                }
            }
            ErrorKind::Connect => {
                self.update_request_status(RequestNetworkStatus::ServerUnreachable);
            }

            _ => {}
        }
    }

    fn on_recv_ok(&self, resp: &ProtonResponse) {
        if resp.is(429) || resp.status().is_server_error() {
            self.update_request_status(RequestNetworkStatus::ServerUnreachable);
        } else {
            self.update_request_status(RequestNetworkStatus::Online);
        }
    }
}

impl SenderLayer<ProtonRequest, ProtonResponse> for ConnectionMonitor {
    fn on_send<'a>(
        &'a self,
        inner: &'a dyn Sender<ProtonRequest, ProtonResponse>,
        req: ProtonRequest,
    ) -> BoxFut<'a, mail_muon::Result<ProtonResponse>> {
        Box::pin(self.on_send(inner, req))
    }
}
