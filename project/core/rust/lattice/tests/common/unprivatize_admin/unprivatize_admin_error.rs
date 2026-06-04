use std::string::FromUtf8Error;

use derive_more::{Display, Error, From};
use lattice::LatticeError;
use lattice_muon2::LtTransportError;
use proton_crypto_account::salts::SaltError;

use super::super::org_members::OrgMemberError;

#[derive(Debug, Display, Error, From)]
pub enum UnprivatizeAdminError {
    #[display("{_0}")]
    Core(#[from] LatticeError),
    #[display("{_0}")]
    Transport(#[from] muon::Error),
    #[display("key passphrase: {_0}")]
    KeyPassphrase(#[from] SaltError),
    #[display("user has no primary PGP key")]
    NoPrimaryUserKey,
    #[from(ignore)]
    #[display("user keys not unlocked: {failed}")]
    UserKeysNotUnlocked {
        #[error(ignore)]
        failed: String,
    },
    #[display("GET /organizations/keys: PrivateKey is absent")]
    NoOrgPrivateKey,
    #[from(ignore)]
    #[display("PGP import/derive: {_0}")]
    PgpImportOrDerive(#[error(ignore)] String),
    #[display("org public key: no SHA-256 fingerprint")]
    NoOrgSha256Fingerprint,
    #[from(ignore)]
    #[display("sign org fingerprint: {_0}")]
    PgpSignFingerprint(#[error(ignore)] String),
    #[from(ignore)]
    #[display("sign invitation: {_0}")]
    PgpSignInvitation(#[error(ignore)] String),
    #[display("user has no addresses")]
    NoAddresses,
    #[display("no primary address key")]
    NoPrimaryAddressKey,
    #[display("armored PGP is not UTF-8: {_0}")]
    PgpArmoredNotUtf8(#[from] FromUtf8Error),
    #[from(ignore)]
    #[display("member {email:?} not in org (member count: {num_members})")]
    MemberNotFound {
        #[error(ignore)]
        email: String,
        #[error(ignore)]
        num_members: usize,
    },
    #[from(ignore)]
    #[display("member {email:?} unprivatization is not Ready")]
    UnprivatizationNotReady {
        #[error(ignore)]
        email: String,
    },
    #[from(ignore)]
    #[display("member {email:?} missing unprivatization activation_token")]
    MissingUnprivActivationToken {
        #[error(ignore)]
        email: String,
    },
    #[from(ignore)]
    #[display("member {email:?} missing unprivatization private keys")]
    MissingUnprivPrivateKeys {
        #[error(ignore)]
        email: String,
    },
}

impl From<LtTransportError> for UnprivatizeAdminError {
    fn from(e: LtTransportError) -> Self {
        match e {
            LtTransportError::Lattice(le) => Self::Core(le),
            LtTransportError::Transport(te) => Self::Transport(te),
        }
    }
}

impl From<OrgMemberError> for UnprivatizeAdminError {
    fn from(e: OrgMemberError) -> Self {
        match e {
            OrgMemberError::Transport(t) => t.into(),
            OrgMemberError::KeyPassphrase(s) => Self::KeyPassphrase(s),
            OrgMemberError::NoPrimaryUserKey => Self::NoPrimaryUserKey,
            OrgMemberError::MemberNotFound { email, num_members } => {
                Self::MemberNotFound { email, num_members }
            }
        }
    }
}
