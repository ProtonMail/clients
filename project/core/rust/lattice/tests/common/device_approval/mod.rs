//! SSO org-managed member device approval test helpers.
//!
//! Flow: user self-approve → admin reset/approve → associate.
//! Depends on [`super::super::sso_setup`], [`super::super::unprivatize_admin`], and
//! [`super::super::sso_member_setup`]. Integration tests require a live atlas stack
//! (`ENV_NAME=davy`, see [`super::super::muon`]).

mod admin_device_approval_error;
mod admin_device_reset;
pub mod device_secret;
mod device_secret_error;
pub mod error;
mod member_approval_keys;
pub mod pending_device;
pub mod pending_device_error;
pub mod sso_org;
pub mod unprivatized_member;
