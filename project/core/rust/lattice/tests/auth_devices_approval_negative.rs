//! Negative paths for SSO device approval helpers (no live stack required).

mod common;

use lattice::auth::LtAuthAddressId;

use crate::common::device_approval::{
    device_secret::DeviceSecret, pending_device::PendingDevice,
    pending_device_error::PendingDeviceError, unprivatized_member::UnprivatizedMember,
};

#[tokio::test]
async fn approve_device_rejects_empty_confirmation_code() {
    let pending = PendingDevice {
        id: "device-id".into(),
        device_token: "token".into(),
        device_secret: DeviceSecret::random(),
        confirmation_code: String::new(),
        activation_address_id: LtAuthAddressId("address-id".into()),
        activation_token: String::new(),
    };

    let member = UnprivatizedMember {
        email: "member@example.com".into(),
        backup_password: "password".into(),
        org_passphrase: proton_crypto_account::salts::KeySecret::new(b"passphrase".to_vec()),
        session: crate::common::generate_muon_session().await,
    };

    let err = member.approve_device(&pending).await.unwrap_err();
    assert!(matches!(err, PendingDeviceError::EmptyConfirmationCode));
}

#[test]
fn pending_device_error_state_mismatch_is_typed() {
    use lattice::auth::devices::LtAuthDeviceState;

    let err = PendingDeviceError::StateMismatch {
        device_id: "id".into(),
        expected: LtAuthDeviceState::Active,
        actual: LtAuthDeviceState::PendingActivation,
    };
    let msg = err.to_string();
    assert!(msg.contains("id"));
    assert!(msg.contains("mismatch"));
}
