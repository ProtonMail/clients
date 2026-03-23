use base64::{Engine, prelude::BASE64_STANDARD};
pub use mail_api_utils::{Paginatable, PaginateOptions, PaginateResponse};
pub use mail_avatar::{first_grapheme_uppercase, proton_color};
pub use mail_shared_types::MapVec;
use proton_crypto::generate_secure_random_bytes;

const NONCE_SIZE: usize = 32;

/// Generate a random nonce for the sake of Content Security Policy.
/// <https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy#nonce-nonce_value/>
#[must_use]
pub fn generate_csp_nonce() -> String {
    let bytes: [u8; NONCE_SIZE] = generate_secure_random_bytes();
    BASE64_STANDARD.encode(bytes)
}
