use std::future::Future;

use lattice::{LtWireRequestProvider, transport::LtTransportProvider};

use crate::{LtQuarkContract, LtQuarkResponseExt, LtQuarkWireExt};

/// Extension on [`LtTransportProvider`] to send Quark commands through the existing wire pipeline.
pub trait LtQuarkTransportProvider: LtTransportProvider
where
    <Self::WireProvider as LtWireRequestProvider>::Error: Into<Self::Error>,
{
    fn send_contract_quark<T: LtQuarkContract>(
        &self,
        contract: &T,
    ) -> impl Future<Output = Result<T::Response, Self::Error>> {
        async move {
            let wire = contract.to_wire_request()?;
            let wire_res = self.send_wire_request(wire).await?;
            wire_res.into_quark_response::<T>().map_err(Into::into)
        }
    }
}

impl<T> LtQuarkTransportProvider for T
where
    T: LtTransportProvider,
    <T::WireProvider as LtWireRequestProvider>::Error: Into<T::Error>,
{
}
