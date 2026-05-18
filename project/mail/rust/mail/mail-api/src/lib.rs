//! Rust bindings for the REST API for Proton

pub mod services;

pub const MAX_PAGE_ELEMENT_COUNT: usize = 200;
pub const MAX_PAGE_ELEMENT_COUNT_U64: u64 = 200;

pub const MAX_LIMIT_VALUE: usize = 150;
pub const MAX_LIMIT_VALUE_U64: u64 = 150;

pub const INCOMING_DEFAULTS_PAGE_SIZE: u64 = 100;

pub use mail_core_api;

#[cfg(test)]
mod integration_tests {
    use {
        bytes as _, reqwest as _, tokio as _, tracing as _, tracing_subscriber as _, wiremock as _,
    };
}
