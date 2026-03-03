//! Unleash feature flag types and public API.
//!
//! This crate provides data types for the Unleash feature flag API
//! (<https://docs.getunleash.io/reference/api/unleash/get-frontend-features/>).
//!
//! No Crux dependencies. Used by mail and other non-Crux consumers.
//! For Crux apps, use `core-feature-flags-op` which adds `UnleashCtx` and related ops.

mod types;

pub use types::*;
