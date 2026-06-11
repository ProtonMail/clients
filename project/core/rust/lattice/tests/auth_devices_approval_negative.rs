//! Negative paths for SSO device approval helpers (no live stack required).

#![recursion_limit = "256"]

mod common;

use core_key::DeviceDisplayCodeError;
use lattice::{Sensitive, auth::LtAuthAddressId, core::LtCoreAuthDeviceId};

use crate::common::device_approval::{
    pending_device::{PendingDevice, random_device_secret},
    pending_device_error::PendingDeviceError,
    unprivatized_member::UnprivatizedMember,
};

#[tokio::test]
async fn approve_device_rejects_empty_confirmation_code() {
    let pending = PendingDevice {
        id: LtCoreAuthDeviceId("device-id".to_string()),
        device_token: "token".into(),
        device_secret: random_device_secret(),
        confirmation_code: String::new(),
        activation_address_id: LtAuthAddressId("address-id".into()),
        activation_token: Sensitive::new(String::new()),
    };

    let member = UnprivatizedMember {
        email: "member@example.com".into(),
        backup_password: "password".into(),
        org_passphrase: proton_crypto_account::salts::KeySecret::new(b"passphrase".to_vec()),
        session: crate::common::generate_muon_session().await,
    };

    let err = member.approve_device(&pending).await.unwrap_err();
    assert!(
        matches!(
            err,
            PendingDeviceError::Crypto(core_key::SharedCryptoError::DisplayCode(
                DeviceDisplayCodeError::WrongLength {
                    expected: 4,
                    actual: 0,
                },
            ))
        ),
        "{:?}",
        err
    );
}
