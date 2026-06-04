use derive_more::{Display, Error, From};
use lattice_muon2::LtTransportError;
use proton_crypto_account::salts::SaltError;

#[derive(Debug, Display, Error, From)]
pub enum OrgMemberError {
    #[display("{_0}")]
    Transport(#[from] LtTransportError),
    #[display("key passphrase: {_0}")]
    KeyPassphrase(#[from] SaltError),
    #[display("user has no primary key")]
    NoPrimaryUserKey,
    #[from(ignore)]
    #[display("member {email:?} not in org (member count: {num_members})")]
    MemberNotFound {
        #[error(ignore)]
        email: String,
        #[error(ignore)]
        num_members: usize,
    },
}
