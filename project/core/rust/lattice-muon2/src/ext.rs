use std::future::Future;

use lattice::LtContract;
use lattice::LtTransportProvider;
use muon::Session;

use crate::{LtTransportError, Muon2Transport};

/// Send a [`LtContract`] using a Muon [`Session`].
pub trait LatticeExt: LtContract + Sized {
    fn send_with<C: muon::Context + Send>(
        &self,
        session: Session<C>,
    ) -> impl Future<Output = Result<Self::Response, LtTransportError>>
    where
        Session<C>: Send + Sync,
    {
        async move {
            Muon2Transport::new(session)
                .send_contract_request(self)
                .await
        }
    }
}

impl<T: LtContract + Sized> LatticeExt for T {}
