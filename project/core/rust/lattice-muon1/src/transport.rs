use lattice::{LtTransportProvider, LtWireRequestProvider};
use mail_muon::{ProtonRequest, ProtonResponse, common::Sender};

use crate::{LtTransportError, Muon1WireRequestProvider};

/// [`lattice::LtTransportProvider`] adapter for a [`mail_muon::common::Sender`].
///
/// Holds a borrowed sender (mail stack sessions are typically used by reference).
pub struct Muon1Transport<'a, S: ?Sized>
where
    S: Sender<ProtonRequest, ProtonResponse> + Send + Sync,
{
    sender: &'a S,
}

impl<'a, S: ?Sized> Muon1Transport<'a, S>
where
    S: Sender<ProtonRequest, ProtonResponse> + Send + Sync,
{
    pub fn new(sender: &'a S) -> Self {
        Self { sender }
    }
}

impl<'a, S: ?Sized> LtTransportProvider for Muon1Transport<'a, S>
where
    S: Sender<ProtonRequest, ProtonResponse> + Send + Sync,
{
    type Error = LtTransportError;
    type WireProvider = Muon1WireRequestProvider;

    async fn send_request(
        &self,
        request: &<Self::WireProvider as LtWireRequestProvider>::Request,
    ) -> Result<<Self::WireProvider as LtWireRequestProvider>::Response, Self::Error> {
        self.sender
            .send(request.clone())
            .await
            .map_err(LtTransportError::Transport)
    }
}
