use lattice::Sensitive;
use lattice::auth::devices::LtAuthDeviceState;
use lattice::auth::devices::LtAuthPostDevicesDeviceIDReq;
use lattice::core::LtCoreAddressesListQuery;
use lattice::core::get_core_addresses::LtCoreGetAddressesReq;
use lattice::core::keys::LtCoreGetKeySaltsReq;
use lattice::core::user::get_users::LtCoreGetUsersReq;
use proton_crypto::new_pgp_provider;
use proton_crypto_account::salts::KeySecret;

use super::super::Session;
use super::super::org_members::{derive_key_passphrase, primary_key_id};
use super::device_secret::DeviceSecret;
use super::error::DeviceApprovalError;
use super::pending_device::PendingDevice;
use super::pending_device_error::PendingDeviceError;

pub struct UnprivatizedMember {
    pub email: String,
    pub backup_password: String,
    pub org_passphrase: KeySecret,
    pub session: Session,
}

impl UnprivatizedMember {
    pub async fn approve_device(&self, pending: &PendingDevice) -> Result<(), PendingDeviceError> {
        if pending.confirmation_code.is_empty() {
            return Err(PendingDeviceError::EmptyConfirmationCode);
        }

        let pgp = new_pgp_provider();
        let user = self.session.send_lt(LtCoreGetUsersReq).await?.user;
        let primary_id = primary_key_id(&user)?;
        let salts = self.session.send_lt(LtCoreGetKeySaltsReq).await?;
        let key_passphrase = derive_key_passphrase(
            &salts.key_salts,
            &primary_id,
            self.backup_password.as_bytes(),
        )
        .map_err(PendingDeviceError::KeyPassphrase)?;

        let user_unlock = user.keys.0.unlock(&pgp, &key_passphrase);
        if user_unlock.unlocked_keys.is_empty() {
            return Err(PendingDeviceError::UserKeysNotUnlocked {
                failed: format!("{:?}", user_unlock.failed),
            });
        }

        let addresses = self
            .session
            .send_lt(LtCoreGetAddressesReq {
                query: LtCoreAddressesListQuery::default(),
            })
            .await?;

        let activation_address = addresses
            .addresses
            .iter()
            .find(|a| a.id == pending.activation_address_id)
            .ok_or(PendingDeviceError::ActivationAddressNotFound)?;

        let addr_unlock = activation_address.keys.0.unlock(
            &pgp,
            &user_unlock.unlocked_keys,
            Some(&key_passphrase),
        );
        if addr_unlock.unlocked_keys.is_empty() {
            return Err(PendingDeviceError::AddressKeysNotUnlocked);
        }

        let address_private_keys: Vec<_> = addr_unlock
            .unlocked_keys
            .iter()
            .map(|k| &k.private_key)
            .collect();

        DeviceSecret::decrypt_activation_token_armored(
            &pgp,
            &address_private_keys,
            &pending.activation_token,
        )?;

        let encrypted_secret = pending
            .device_secret
            .encrypt_passphrase(key_passphrase.as_ref())?;

        self.session
            .send_lt(LtAuthPostDevicesDeviceIDReq {
                device_id: pending.id.clone(),
                encrypted_secret: Sensitive::new(encrypted_secret),
            })
            .await?;

        Ok(())
    }

    pub async fn complete_user_device(
        &self,
        name: &str,
    ) -> Result<PendingDevice, DeviceApprovalError> {
        let pending = PendingDevice::register(&self.session, name).await?;
        pending
            .expect_state_on(&self.session, LtAuthDeviceState::PendingActivation)
            .await?;
        self.approve_device(&pending).await?;
        pending
            .expect_state_on(&self.session, LtAuthDeviceState::Active)
            .await?;
        pending.associate(&self.session).await?;
        Ok(pending)
    }
}
