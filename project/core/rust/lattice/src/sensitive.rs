//! Re-export shared [`Sensitive`] / [`Zeroize`] from [`core_sensitive_data`].
//!
//! [`Sensitive`] implements `Serialize`/`Deserialize` via `core-sensitive-data`'s **`serde`** feature,
//! which lattice always enables.

pub use core_sensitive_data::{Sensitive, Zeroize};
