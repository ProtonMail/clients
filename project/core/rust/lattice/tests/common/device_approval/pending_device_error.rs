use derive_more::{Display, Error, From};
use lattice::core::LtCoreAuthDeviceId;
use lattice_muon2::LtTransportError;
use proton_crypto_account::salts::SaltError;

use core_key::SharedCryptoError;

use crate::common::org_members::OrgMemberError;

#[derive(Debug, Display, Error, From)]
pub enum PendingDeviceError {
    #[display("{_0}")]
    Transport(#[from] LtTransportError),
    #[display("user has no addresses")]
    NoAddresses,
    #[display("no primary public key for address")]
    NoPrimaryPublicKey,
    #[display("missing device_token on created device")]
    MissingDeviceToken,
    #[display("missing activation_address_id")]
    MissingActivationAddressId,
    #[display("device {device_id} state mismatch: expected {expected:?}, got {actual:?}")]
    StateMismatch {
        device_id: LtCoreAuthDeviceId,
        expected: lattice::auth::devices::LtAuthDeviceState,
        actual: lattice::auth::devices::LtAuthDeviceState,
    },
    #[from(ignore)]
    #[display("device {device_id} not found")]
    DeviceNotFound {
        #[error(ignore)]
        device_id: LtCoreAuthDeviceId,
    },
    #[display("activation address not found")]
    ActivationAddressNotFound,
    #[display("activation address keys not unlocked")]
    AddressKeysNotUnlocked,
    #[from(ignore)]
    #[display("user keys not unlocked: {failed}")]
    UserKeysNotUnlocked {
        #[error(ignore)]
        failed: String,
    },
    #[display("associate poll exhausted after {attempts} attempts")]
    AssociatePollExhausted {
        attempts: u32,
        #[error(source)]
        last: Option<LtTransportError>,
    },
    #[display("key passphrase: {_0}")]
    KeyPassphrase(#[from] SaltError),
    #[display("{_0}")]
    Crypto(#[from] SharedCryptoError),
    #[display("org member: {_0}")]
    OrgMember(#[from] OrgMemberError),
}
