//! Muon v2 (`muon` crate) integration for [`lattice`] contracts.
//!
//! Provides:
//! - [`lattice::LtTransportProvider`] for any `SendRequest<HttpReq, HttpRes, …>` sender ([`Muon2Transport`]).
//! - [`lattice::LtWireRequestProvider`] for muon HTTP ([`Muon2WireRequestProvider`]).
//! - [`LatticeExt`] trait for sending [`lattice::LtContract`]s using a [`muon::Session`] (see [`LatticeExt`]).
//!   This trait provides: [`LatticeExt::send_with`] method that sends the contract using the [`Muon2Transport`].
//! - [`LtTransportError`] type for errors that can occur when sending or running a contract.
//!   This error type is a combination of:
//!   - [`muon::Error`] for transport errors.
//!   - [`lattice::LatticeError`] for lattice errors.
//! - [`Muon2Transport`] type for the transport implementation.
//! - [`Muon2WireRequestProvider`] type for the wire request provider implementation.

mod error;
pub use crate::error::LtTransportError;

mod transport;
pub use crate::transport::Muon2Transport;

mod wire;
pub use crate::wire::Muon2WireRequestProvider;

mod ext;
pub use crate::ext::LatticeExt;
