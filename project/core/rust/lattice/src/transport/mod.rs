//! Transport-neutral HTTP wire layer for [`crate::LtContract`] contracts.
//!
//! This module sits between **contract definitions** ([`crate::LtContract`]) and **concrete HTTP
//! clients** (`lattice-muon1`, `lattice-muon2`). It does not depend on `muon` or `mail-muon`.
//!
//! # Flow
//!
//! ```text
//! LtContract  --from_contract-->  LtWireRequest  --from_wire-->  native request
//!                                                                      |
//!                                                                      v
//! LtResponse  <--into_contract_response--  LtWireResponse  <--to_wire--  native response
//! ```
//!
//! [`LtTransportProvider`] implements the full pipeline via [`LtTransportProvider::send_contract_request`].
//! A concrete adapter only implements [`LtTransportProvider::send_request`] plus
//! [`LtWireRequestProvider`] for its HTTP stack.
//!
//! # Main types
//!
//! - [`LtWireRequest`] — method, path, query, headers (built from [`crate::LtContract`] or Quark).
//!   Header values, query values, and request bodies use [`crate::Sensitive`].
//! - [`LtWireResponse`] — status, headers, body bytes (header values and body are [`crate::Sensitive`]).
//! - [`LtWireMethod`] — GET / POST / PUT / DELETE plus optional body.
//! - [`LtWireRequestProvider`] — maps wire ↔ native request/response types.
//! - [`LtTransportProvider`] — sends native requests and parses SlimAPI / Quark responses.
//!
//! Quark commands are provided by the separate `lattice-quark` crate.
//!
//! # Muon adapters
//!
//! | Crate | Native stack |
//! |-------|----------------|
//! | `lattice-muon1` | `mail-muon` (`Muon1Transport`, `Muon1WireRequestProvider`) |
//! | `lattice-muon2` | `muon` v2 (`Muon2Transport`, `Muon2WireRequestProvider`) |
//!
mod provider;
mod wire_method;
mod wire_request;
mod wire_response;

pub use provider::{LtTransportProvider, LtWireRequestProvider};
pub use wire_method::LtWireMethod;
pub use wire_request::LtWireRequest;
pub use wire_response::LtWireResponse;
