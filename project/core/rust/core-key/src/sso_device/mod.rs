//! SSO auth-device crypto shared by account-crux and lattice integration tests.

mod build_manual_unpriv_completion;
mod device_secret;
mod display_code;
mod encrypted_secret;
mod org_admin_pgp;

pub use build_manual_unpriv_completion::LtCoreMemberListUnprivatizationExt;
pub use device_secret::DeviceSecret;
pub use display_code::{DeviceDisplayCode, DeviceDisplayCodeError};
pub use encrypted_secret::{ENCRYPTED_SECRET_CONTEXT, EncryptedSecret};
pub use org_admin_pgp::OrgAdminPgp;

use data_encoding::HEXLOWER;
use proton_crypto::generate_secure_random_bytes;
use proton_crypto_account::salts::KeySecret;

/// 32 bytes encoded as hex (`KeySecret`), matching backend `CoreUtils::generateHexToken(32)`.
pub fn key_secret_from_32_bytes(bytes: [u8; 32]) -> KeySecret {
    KeySecret::new(HEXLOWER.encode(bytes.as_ref()).into_bytes())
}

/// 32 random bytes encoded as hex (`KeySecret`), matching backend `CoreUtils::generateHexToken(32)`.
pub fn secure_hex_key_secret_32() -> KeySecret {
    key_secret_from_32_bytes(generate_secure_random_bytes::<32>())
}
