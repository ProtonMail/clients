use lattice::{LtTransportProvider, LtWireRequestProvider};
use muon::SendRequest;
use muon::http::{HttpReq, HttpRes};

use crate::{LtTransportError, Muon2WireRequestProvider};

pub struct Muon2Transport<
    Sender: SendRequest<HttpReq, HttpRes, Err: Into<muon::Error> + Send> + Send + Sync,
> {
    sender: Sender,
}

impl<Sender: SendRequest<HttpReq, HttpRes, Err: Into<muon::Error> + Send> + Send + Sync>
    Muon2Transport<Sender>
{
    pub fn new(sender: Sender) -> Self {
        Self { sender }
    }
}

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
        let response = self
            .sender
            .send(request)
            .await
            .map_err(|e| LtTransportError::Transport(e.into()))?;
        Ok(response)
    }
}
