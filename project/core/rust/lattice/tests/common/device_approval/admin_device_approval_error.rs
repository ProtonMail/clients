use derive_more::{Display, Error, From};
use lattice_muon2::LtTransportError;
use proton_crypto_account::salts::SaltError;

use super::super::org_members::OrgMemberError;
use super::super::unprivatize_admin::UnprivatizeAdminError;
use super::device_secret_error::DeviceSecretError;

#[derive(Debug, Display, Error, From)]
pub enum AdminDeviceApprovalError {
    #[display("{_0}")]
    Transport(#[from] LtTransportError),
    #[display("{_0}")]
    Setup(#[from] UnprivatizeAdminError),
    #[from(ignore)]
    #[display("{_0}")]
    Unlock(#[error(ignore)] String),
    #[from(ignore)]
    #[display("pgp: {_0}")]
    Pgp(#[error(ignore)] String),
    #[from(ignore)]
    #[display("missing {field}")]
    MissingField { field: &'static str },
    #[from(ignore)]
    #[display("member user key {key_id} missing org token (activation)")]
    MissingOrgToken {
        #[error(ignore)]
        key_id: String,
    },
    #[from(ignore)]
    #[display("no unlocked address key for activation address {activation_address_id:?}")]
    NoDecryptKeysForActivation {
        #[error(ignore)]
        activation_address_id: String,
    },
    #[display("invalid confirmation code")]
    InvalidConfirmationCode,
    #[display("key passphrase: {_0}")]
    KeyPassphrase(#[from] SaltError),
    #[display("{_0}")]
    Org(#[from] OrgMemberError),
    #[display("{_0}")]
    Crypto(#[from] DeviceSecretError),
}
