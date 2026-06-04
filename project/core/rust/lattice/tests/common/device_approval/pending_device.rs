use std::time::Duration;

use lattice::auth::LtAuthAddressId;
use lattice::auth::devices::{
    LtAuthDevice, LtAuthDeviceState, LtAuthPostDevicesAssociateReq, LtAuthPostDevicesCreateReq,
    LtAuthPutDevicesDeviceIDAdminReq,
};
use lattice::core::LtCoreAddressesListQuery;
use lattice::core::get_core_addresses::LtCoreGetAddressesReq;
use lattice::core::get_keys_all::LtCoreGetKeysAllReq;
use lattice::{LatticeError, LtApiResponseError};
use lattice_muon2::LtTransportError;
use proton_crypto::new_pgp_provider;
use tokio::time::sleep;

use super::super::Session;
use super::device_secret::DeviceSecret;
use super::pending_device_error::PendingDeviceError;

pub struct PendingDevice {
    pub id: String,
    pub device_token: String,
    pub device_secret: DeviceSecret,
    pub confirmation_code: String,
    pub activation_address_id: LtAuthAddressId,
    pub activation_token: String,
}

impl PendingDevice {
    pub async fn register(session: &Session, name: &str) -> Result<Self, PendingDeviceError> {
        let addresses = session
            .send_lt(LtCoreGetAddressesReq {
                query: LtCoreAddressesListQuery::default(),
            })
            .await?;
        let primary_address = addresses
            .addresses
            .iter()
            .min_by_key(|a| a.order)
            .ok_or(PendingDeviceError::NoAddresses)?;

        let keys = session
            .send_lt(LtCoreGetKeysAllReq {
                email: primary_address.email.clone(),
            })
            .await?;
        let primary_pubkey = keys
            .address_keys
            .keys
            .iter()
            .find(|k| k.primary)
            .ok_or(PendingDeviceError::NoPrimaryPublicKey)?;

        let device_secret = DeviceSecret::random();
        let pgp = new_pgp_provider();
        let activation_token =
            device_secret.encrypt_activation_token(&pgp, &primary_pubkey.public_key)?;

        let create_res = session
            .send_lt(LtAuthPostDevicesCreateReq {
                name: name.to_string(),
                activation_token: Some(activation_token.clone()),
            })
            .await?;

        let device = create_res.auth_device;
        let device_token = device
            .device_token
            .clone()
            .ok_or(PendingDeviceError::MissingDeviceToken)?;
        let activation_address_id = device
            .activation_address_id
            .clone()
            .ok_or(PendingDeviceError::MissingActivationAddressId)?;

        Ok(Self {
            id: device.id,
            device_token,
            confirmation_code: device_secret.display_code(),
            device_secret,
            activation_address_id: LtAuthAddressId(activation_address_id),
            activation_token,
        })
    }

    pub async fn expect_state_on(
        &self,
        session: &Session,
        expected: LtAuthDeviceState,
    ) -> Result<(), PendingDeviceError> {
        let devices = session.auth_devices().await?;
        let device = devices.iter().find(|d| d.id == self.id).ok_or_else(|| {
            PendingDeviceError::DeviceNotFound {
                device_id: self.id.clone(),
            }
        })?;
        if device.state != expected {
            return Err(PendingDeviceError::StateMismatch {
                device_id: self.id.clone(),
                expected,
                actual: device.state,
            });
        }
        Ok(())
    }

    pub async fn associate(&self, session: &Session) -> Result<(), PendingDeviceError> {
        const MAX_ATTEMPTS: u32 = 30;
        let mut last_transient_err = None;
        for attempt in 0..MAX_ATTEMPTS {
            match session
                .send_lt(LtAuthPostDevicesAssociateReq {
                    device_id: self.id.clone(),
                    device_token: self.device_token.clone(),
                })
                .await
            {
                Ok(associate) => {
                    self.device_secret
                        .decrypt_encrypted_secret(&associate.auth_device.encrypted_secret)?;
                    return Ok(());
                }
                Err(e) if Self::is_associate_transient(&e) => {
                    last_transient_err = Some(e);
                    if attempt + 1 >= MAX_ATTEMPTS {
                        break;
                    }
                    sleep(Duration::from_millis(500)).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(PendingDeviceError::AssociatePollExhausted {
            attempts: MAX_ATTEMPTS,
            last: last_transient_err,
        })
    }

    fn is_associate_transient(err: &LtTransportError) -> bool {
        matches!(
            err,
            LtTransportError::Lattice(LatticeError::ApiError(_, api_err))
                if matches!(api_err.as_ref(), LtApiResponseError::DeviceNotActive(_))
        )
    }

    pub async fn expect_absent_from_admin_pending(
        &self,
        admin_session: &Session,
    ) -> Result<(), PendingDeviceError> {
        let pending = admin_session.admin_pending_devices().await?;
        assert!(
            !pending.iter().any(|d| d.id == self.id),
            "expected device {} absent from admin pending list, got {pending:?}",
            self.id
        );
        Ok(())
    }

    pub async fn fetch_admin_pending_row(
        &self,
        admin_session: &Session,
    ) -> Result<LtAuthDevice, PendingDeviceError> {
        admin_session
            .admin_pending_devices()
            .await?
            .into_iter()
            .find(|d| d.id == self.id)
            .ok_or_else(|| PendingDeviceError::DeviceNotFound {
                device_id: self.id.clone(),
            })
    }

    pub async fn request_admin_activation(
        &self,
        session: &Session,
    ) -> Result<(), PendingDeviceError> {
        session
            .send_lt(LtAuthPutDevicesDeviceIDAdminReq {
                device_id: self.id.clone(),
            })
            .await?;
        Ok(())
    }
}
