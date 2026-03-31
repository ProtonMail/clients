#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::must_use_candidate)]

pub mod cache;
pub mod error;
mod ids;
mod manager;
mod policy;
pub mod traits;

pub use ids::*;
pub use manager::*;
pub use policy::*;

pub type Result<T> = std::result::Result<T, error::KeyHandlingError>;

/// Re-export the `proton-crypto-account` dependency.
pub use proton_crypto_account;
