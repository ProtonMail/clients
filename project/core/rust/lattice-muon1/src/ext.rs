use std::future::Future;

use lattice::LtContract;
use lattice::LtTransportProvider;
use mail_muon::common::Sender;
use mail_muon::{ProtonRequest, ProtonResponse};

use crate::{LtTransportError, Muon1Transport};

/// Adds a `send_with` method to [`LtContract`] that sends the contract using a mail-muon [`Sender`].
pub trait LatticeExt: LtContract + Sized {
    fn send_with<S: Sender<ProtonRequest, ProtonResponse> + Sync>(
        &self,
        sender: &S,
    ) -> impl Future<Output = Result<Self::Response, LtTransportError>> {
        async {
            Muon1Transport::new(sender)
                .send_contract_request(self)
                .await
        }
    }
}

impl<T: LtContract + Sized> LatticeExt for T {}

/// Run a [`LtContract`] using a mail-muon [`Sender`].
pub trait RunLatticeContractExt: Sender<ProtonRequest, ProtonResponse> + Sync {
    fn run_lattice_contract<T: LtContract>(
        &self,
        contract: &T,
    ) -> impl Future<Output = Result<T::Response, LtTransportError>> {
        async {
            Muon1Transport::new(self)
                .send_contract_request(contract)
                .await
        }
    }
}

impl<S: ?Sized + Sender<ProtonRequest, ProtonResponse> + Sync> RunLatticeContractExt for S {}
