//! Foundational Proton ID types and privacy wrappers.

mod macros;
mod private_data;

pub use private_data::{PrivateEmail, PrivateEmailRef, PrivateString};

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

/// Marker trait for Proton remote ID newtypes produced by [`declare_proton_id!`].
pub trait ProtonIdMarker:
    Deref<Target = str>
    + Clone
    + Debug
    + DeserializeOwned
    + Eq
    + Hash
    + PartialEq
    + Serialize
    + Sync
    + Send
{
}
