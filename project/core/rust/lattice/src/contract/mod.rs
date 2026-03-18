//! Lattice contract traits for defining HTTP request/response contracts.
//!
//! This module contains the core traits for defining contracts with the Lattice API:
//!
//! - [`LtContract`]: Standard contract trait
//! - [`AuthReq`]: Marker trait for authenticated requests
//! - [`UnauthReq`]: Marker trait for unauthenticated requests
//!
//! ## Authentication Markers
//!
//! Use [`AuthReq`] and [`UnauthReq`] marker traits to indicate whether a contract
//! requires authentication. This helps with type-safe session handling.

mod lt_contract;

pub use lt_contract::LtContract;
use serde::{Deserialize, Serialize};

use crate::LatticeError;

/// A trait for Lattice contracts that are not authenticated.
///
/// This trait is implemented by all Lattice contracts that don't require authentication.
pub trait UnauthReq {}

/// A trait for Lattice contracts that are authenticated.
///
/// This trait is implemented by all Lattice contracts that require authentication.
pub trait AuthReq {}

pub trait LtRequestBody {
    fn to_body(&self) -> Result<Vec<u8>, LatticeError>;
}

pub trait LtResponseBody: Sized {
    fn from_body(body: &[u8]) -> Result<Self, LatticeError>;
}

pub struct LtRawBody(pub Vec<u8>);

impl LtRequestBody for LtRawBody {
    fn to_body(&self) -> Result<Vec<u8>, LatticeError> {
        Ok(self.0.clone())
    }
}

impl LtResponseBody for LtRawBody {
    fn from_body(body: &[u8]) -> Result<Self, LatticeError> {
        Ok(LtRawBody(body.to_vec()))
    }
}

/// A body type for slimAPI Lattice contracts that use JSON for the request body.
///
/// When used as a request body, the type needs to implement `Serialize` from `serde`.
/// When used as a response body, the type needs to implement `Deserialize` from `serde`.
///
/// For general purpose JSON contracts, use [`LtJson`].
pub struct LtSlimAPIJSON<T>(pub T);

#[cfg(feature = "serde")]
impl<T: Serialize> LtRequestBody for LtSlimAPIJSON<T> {
    fn to_body(&self) -> Result<Vec<u8>, LatticeError> {
        serde_json::to_vec(&self.0).map_err(|e| {
            LatticeError::SerdeJSON(
                e,
                Some(format!(
                    "Cannot serialize body of type {:?}",
                    std::any::type_name::<T>()
                )),
            )
        })
    }
}

#[cfg(feature = "serde")]
impl<T: for<'de> Deserialize<'de>> LtResponseBody for LtSlimAPIJSON<T> {
    fn from_body(body: &[u8]) -> Result<Self, LatticeError> {
        let response: crate::LtApiResponse<T> = serde_json::from_slice(body)
            .map_err(|e| LatticeError::SerdeJSON(e, String::from_utf8(body.to_vec()).ok()))?;
        Ok(LtSlimAPIJSON(response.body))
    }
}

/// A body type for JSON Lattice contracts.
///
/// When used as a request body, the type needs to implement `Serialize` from `serde`.
/// When used as a response body, the type needs to implement `Deserialize` from `serde`.
///
/// WARNING: This is not a slimAPI contract. It is a general purpose JSON contract.
/// For slimAPI contracts, use [`LtSlimAPIJSON`].
pub struct LtJson<T>(pub T);

#[cfg(feature = "serde")]
impl<T: Serialize> LtRequestBody for LtJson<T> {
    fn to_body(&self) -> Result<Vec<u8>, LatticeError> {
        serde_json::to_vec(&self.0).map_err(|e| {
            LatticeError::SerdeJSON(
                e,
                Some(format!(
                    "Cannot serialize body of type {:?}",
                    std::any::type_name::<T>()
                )),
            )
        })
    }
}

#[cfg(feature = "serde")]
impl<T: for<'de> Deserialize<'de>> LtResponseBody for LtJson<T> {
    fn from_body(body: &[u8]) -> Result<Self, LatticeError> {
        Ok(LtJson(serde_json::from_slice(body).map_err(|e| {
            LatticeError::SerdeJSON(e, String::from_utf8(body.to_vec()).ok())
        })?))
    }
}
