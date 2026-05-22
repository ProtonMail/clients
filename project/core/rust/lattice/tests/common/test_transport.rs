use lattice::{LatticeError, LtApiResponseError, LtContract};
use lattice::{LtTransportProvider, LtWireRequestProvider};
use lattice_muon2::{LtTransportError, Muon2Transport, Muon2WireRequestProvider};
use lattice_quark::{LtQuarkTransportProvider, jail::unban::LtQuarkJailUnban};

use crate::common::MuonCtx;

/// Layer over `Muon2Transport` to unban the session if it fails with a human verification error.
/// It uses quark commands to unban the session.
pub struct Muon2TestTransport(Muon2Transport<muon::Session<MuonCtx>>);

impl Muon2TestTransport {
    pub fn new(session: muon::Session<MuonCtx>) -> Self {
        Self(Muon2Transport::new(session))
    }
}

impl LtTransportProvider for Muon2TestTransport {
    type Error = LtTransportError;
    type WireProvider = Muon2WireRequestProvider;

    async fn send_contract_request<T: LtContract>(
        &self,
        contract: &T,
    ) -> Result<T::Response, Self::Error> {
        match self.0.send_contract_request(contract).await {
            Ok(res) => Ok(res),
            // If the request fails with a human verification error, unban the session and try again.
            Err(LtTransportError::Lattice(LatticeError::ApiError(_, b)))
                if matches!(b.as_ref(), LtApiResponseError::HumanVerification(..)) =>
            {
                self.send_contract_quark(&LtQuarkJailUnban).await?;
                // Here we shouldn't call self.send_contract_request again because it will loop forever.
                // if the unban fails.
                self.0.send_contract_request(contract).await
            }
            Err(e) => Err(e),
        }
    }

    fn send_request(
        &self,
        request: &<Self::WireProvider as LtWireRequestProvider>::Request,
    ) -> impl Future<Output = Result<<Self::WireProvider as LtWireRequestProvider>::Response, Self::Error>>
    {
        self.0.send_request(request)
    }
}
