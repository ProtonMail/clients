#![recursion_limit = "256"]

#[macro_use]
#[path = "../common/mod.rs"]
mod common;
#[path = "../common_sso/mod.rs"]
mod common_sso;

mod auth;
mod auth_devices;
mod auth_devices_approval;
mod auth_devices_approval_negative;
mod errors;
mod organization;
mod quark;
mod sso;
mod unprivatization;
mod user;
mod user_settings;
