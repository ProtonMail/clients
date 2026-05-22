use std::future::Future;

use crate::{LatticeError, LtContract};

use super::wire_request::LtWireRequest;
use super::wire_response::LtWireResponse;

pub trait LtWireRequestProvider {
    type Request: Send;
    type Response: Send;
    type Error: Send;

    fn from_wire(wire: LtWireRequest) -> Result<Self::Request, Self::Error>;
    fn to_wire(res: Self::Response) -> Result<LtWireResponse, Self::Error>;
}

pub trait LtTransportProvider: Sized + Send
where
    <Self::WireProvider as LtWireRequestProvider>::Error: Into<Self::Error>,
{
    type Error: From<LatticeError> + std::error::Error + Send + Sync + 'static;
    type WireProvider: LtWireRequestProvider;

    fn send_request(
        &self,
        request: &<Self::WireProvider as LtWireRequestProvider>::Request,
    ) -> impl Future<Output = Result<<Self::WireProvider as LtWireRequestProvider>::Response, Self::Error>>;

    fn send_wire_request(
        &self,
        wire: LtWireRequest,
    ) -> impl Future<Output = Result<LtWireResponse, Self::Error>> {
        async move {
            let native = <Self::WireProvider as LtWireRequestProvider>::from_wire(wire)
                .map_err(Into::into)?;
            let res = self.send_request(&native).await?;
            <Self::WireProvider as LtWireRequestProvider>::to_wire(res).map_err(Into::into)
        }
    }
    fn send_contract_request<T: LtContract>(
        &self,
        contract: &T,
    ) -> impl Future<Output = Result<T::Response, Self::Error>> {
        async move {
            let wire = LtWireRequest::from_contract(contract)?;
            let wire_res = self.send_wire_request(wire).await?;
            wire_res.into_contract_response::<T>().map_err(Into::into)
        }
    }
}
