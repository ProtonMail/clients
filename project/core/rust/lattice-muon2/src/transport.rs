use lattice::{LtTransportProvider, LtWireRequestProvider};
#[allow(deprecated)] // Requires SDK support to migrate. See UFC-158
use muon::SendRequest;
use muon::http::{HttpReq, HttpRes};

use crate::{LtTransportError, Muon2WireRequestProvider};

#[allow(deprecated)] // Requires SDK support to migrate. See UFC-158
pub struct Muon2Transport<
    Sender: SendRequest<HttpReq, HttpRes, Err: Into<muon::Error> + Send> + Send + Sync,
> {
    sender: Sender,
}

#[allow(deprecated)] // Requires SDK support to migrate. See UFC-158
impl<Sender: SendRequest<HttpReq, HttpRes, Err: Into<muon::Error> + Send> + Send + Sync>
    Muon2Transport<Sender>
{
    pub fn new(sender: Sender) -> Self {
        Self { sender }
    }
}

#[allow(deprecated)] // Requires SDK support to migrate. See UFC-158
impl<Sender: SendRequest<HttpReq, HttpRes, Err: Into<muon::Error> + Send> + Send + Sync>
    LtTransportProvider for Muon2Transport<Sender>
{
    type Error = LtTransportError;
    type WireProvider = Muon2WireRequestProvider;

    async fn send_request(
        &self,
        request: &<Self::WireProvider as LtWireRequestProvider>::Request,
    ) -> Result<<Self::WireProvider as LtWireRequestProvider>::Response, Self::Error> {
        let request = request.clone();
        #[allow(deprecated)] // Requires SDK support to migrate. See UFC-158
        let response = self
            .sender
            .send(request)
            .await
            .map_err(|e| LtTransportError::Transport(e.into()))?;
        Ok(response)
    }
}
