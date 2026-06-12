//! Shared email packaging for Proton applications.
//!
//! This crate builds the encrypted `Package` structs that the Proton send APIs
//! consume. It does not drive sending — callers (mail's draft+send flow, mail's
//! RSVP direct-send flow, calendar's sync engine) keep their own send logic and
//! invoke `build_packages()` for the crypto step.
//!
//! # Recommended entry points
//!
//! Most callers should not invoke [`build_packages`] directly. Use the
//! integrator wrappers, which fill in the app-specific glue (key loading,
//! send-preferences resolution, EO provider) for you:
//!
//! - **Mail draft + send flow** — `mail_common::draft::send::build_packages`.
//! - **Direct send / calendar** — a forthcoming `send_email()` facade will
//!   live alongside this crate; until then, calendar consumers should
//!   coordinate with the platform team before depending on `build_packages`
//!   directly.
//!
//! Calling `build_packages` directly is supported, but you are responsible
//! for preparing every input (unlocked address keys, resolved
//! `SendPreferences`, EO provider) yourself.
//!
//! # What it does
//!
//! Given pre-loaded inputs (recipient encryption preferences, message body,
//! attachments as bytes, sender keys), it encrypts the body and attachments
//! per recipient and produces `Package` structs ready for the Proton send API.
//!
//! # What it does NOT do directly
//!
//! - **No direct I/O**: no HTTP client, no session, no database, no filesystem
//! - **No key resolution**: use `mail-core-key-manager` to obtain `SendPreferences`
//! - **No retry/queue**: the caller handles retries and action queuing
//!
//! The only I/O the crate may perform is delegated through the
//! `EoModulusProvider` trait when an Encrypted Outside recipient is present;
//! see below.
//!
//! # Encrypted Outside (EO)
//!
//! Password-protected ("Encrypt to Outside") recipients are supported. The
//! caller supplies `EoData` (password + optional hint) and an
//! `EoModulusProvider`; when an EO recipient is detected, the crate awaits one
//! call to the provider's `get_auth_modulus()` and proceeds with the SRP
//! challenge internally. Callers with no EO recipients may pass `None` for
//! both — the provider is never invoked.
//!
//! # Usage
//!
//! ```ignore
//! use mail_package_builder::{build_packages, BodyFormat, SendType, LoadedAttachment};
//!
//! let packages = build_packages(
//!     &pgp,
//!     SendType::Direct,
//!     &address_keys,
//!     &send_preferences,
//!     "Hello, world!",
//!     BodyFormat::PlainText,
//!     &attachments,
//!     None, // eo_data (no EO recipients)
//!     None, // eo_modulus_provider (no EO recipients)
//! ).await?;
//! ```

mod attachment_entries;
mod body_convert;
mod eo;
mod error;
mod packages;
mod types;

pub use attachment_entries::PackageAttachmentEntries;
pub use body_convert::html_to_text;
pub use eo::{EoModulus, EoModulusProvider, NoopEoModulusProvider};
pub use error::PackageError;
pub use packages::build_packages;
pub use types::{
    AttachmentDisposition, BodyFormat, EoContainer, EoData, LoadedAttachment, SendType,
};
