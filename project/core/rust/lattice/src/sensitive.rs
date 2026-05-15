//! Re-export shared [`Sensitive`] / [`Zeroize`] from [`core_sensitive_data`].
//!
//! Enable lattice's **`serde`** feature for `Serialize`/`Deserialize` on [`Sensitive`]. Crates that
//! need Facet (e.g. account-crux) should depend on `core-sensitive-data` with **`serde`** and
//! **`facet`** so the unified [`Sensitive`] type implements [`facet::Facet`].

pub use core_sensitive_data::{Sensitive, Zeroize};
