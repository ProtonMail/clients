//! Muon v1 (`mail-muon` crate) integration for [`lattice`] contracts.
//!
//! Provides:
//! - [`lattice::LtTransportProvider`] implementation for a [`mail_muon::common::Sender`] (see [`Muon1Transport`]).
//! - [`lattice::LtWireRequestProvider`] implementation for the mail-muon transport (see [`Muon1WireRequestProvider`]).
//! - [`LatticeExt`] trait for sending [`lattice::LtContract`]s using a [`mail_muon::common::Sender`] (see [`LatticeExt`]).
//!   This trait provides: [`LatticeExt::send_with`] method that sends the contract using the [`Muon1Transport`].
//! - [`RunLatticeContractExt`] trait for running [`lattice::LtContract`]s using a [`mail_muon::common::Sender`] (see [`RunLatticeContractExt`]).
//!   This trait provides: [`RunLatticeContractExt::run_lattice_contract`] method that runs the contract using the [`Muon1Transport`].
//! - [`LtTransportError`] type for errors that can occur when sending or running a contract.
//!   This error type is a combination of:
//!   - [`mail_muon::Error`] for transport errors.
//!   - [`lattice::LatticeError`] for lattice errors.
//! - [`Muon1Transport`] type for the transport implementation.
//! - [`Muon1WireRequestProvider`] type for the wire request provider implementation.

mod error;
pub use crate::error::LtTransportError;

mod transport;
pub use crate::transport::Muon1Transport;

mod wire;
pub use crate::wire::Muon1WireRequestProvider;

mod ext;
pub use crate::ext::{LatticeExt, RunLatticeContractExt};
