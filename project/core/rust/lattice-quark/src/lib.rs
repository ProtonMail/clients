//! Quark command contracts and lattice transport extensions.
//!
//! Response stdout is parsed via [`LtQuarkRes`] adapters in [`command`] (JSON object, plain text
//! line, raw string, or custom multi-line text). See that module for a format cheat sheet.
//!
//! Quark types live in submodules (`user`, `payments`, `event`, etc.). Sending commands uses
//! extension traits on [`lattice::transport`] types:
//!
//! - [`LtQuarkWireExt::to_wire_request`] — contract → [`LtWireRequest`]
//! - [`LtQuarkResponseExt::into_quark_response`] — [`LtWireResponse`] → typed Quark response
//! - [`LtQuarkTransportProvider::send_contract_quark`] — full pipeline on any [`LtTransportProvider`]
//!
//! Depend on `lattice-quark` alongside a muon adapter crate (e.g. `lattice-muon2`) when tests or
//! tooling need Quark; muon crates do not expose a `quark` feature.

pub use lattice::LatticeError;

pub mod command;
pub mod encryption;
pub mod event;
pub mod jail;
pub mod payments;
pub mod transport;
pub mod user;

pub use command::{
    LtQuarkContract, LtQuarkFormat, LtQuarkJSONRes, LtQuarkRes, LtQuarkResString,
    LtQuarkResTryFrom, QuarkCommand,
};

pub use lattice::transport::{LtTransportProvider, LtWireRequest, LtWireResponse};
pub use transport::{LtQuarkResponseExt, LtQuarkTransportProvider, LtQuarkWireExt};
