use derive_more::{Display, Error, From};
use lattice_muon2::LtTransportError;
use proton_crypto_account::salts::SaltError;

use super::super::org_members::OrgMemberError;
use super::super::unprivatize_admin::UnprivatizeAdminError;
use core_key::SharedCryptoError;

#[derive(Debug, Display, Error, From)]
pub enum AdminDeviceApprovalError {
    #[display("{_0}")]
    Transport(#[from] LtTransportError),
    #[display("{_0}")]
    Setup(#[from] UnprivatizeAdminError),
    #[from(ignore)]
    #[display("missing {field}")]
    MissingField { field: &'static str },
    #[display("key passphrase: {_0}")]
    KeyPassphrase(#[from] SaltError),
    #[display("{_0}")]
    Org(#[from] OrgMemberError),
    #[display("{_0}")]
    Crypto(#[from] SharedCryptoError),
}
