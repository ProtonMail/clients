use lattice::Sensitive;
use lattice::auth::devices::LtAuthDevice;
use lattice::core::get_members::LtCoreGetMembersReq;
use lattice::core::keys::LtCoreGetKeySaltsReq;
use lattice::core::members::devices::{
    LtCorePostMembersDevicesResetBody, LtCorePostMembersDevicesResetReq,
};
use lattice::core::user::get_users::LtCoreGetUsersReq;
use lattice::core::{LtCoreAuthDeviceId, LtCoreMemberEncId};
use proton_crypto::crypto::PGPProviderSync;
use proton_crypto_account::keys::UserKeys;
use proton_crypto_account::salts::KeySecret;

use super::super::Session;
use super::super::org_members::{
    derive_key_passphrase, fetch_member_addresses_paginated, find_member_by_email, primary_key_id,
};
use super::super::unprivatize_admin::AdminPgpState;
use super::admin_device_approval_error::AdminDeviceApprovalError;
use super::device_secret::DeviceSecret;
use super::member_approval_keys::{
    MemberApprovalKeys, decryption_keys_for_activation, device_secret_from_activation,
    rearmor_user_keys, unlock_member_approval_keys,
};

impl<P: PGPProviderSync> AdminPgpState<P> {
    pub async fn approve_member_device(
        &self,
        admin_session: &Session,
        member_salts_session: &Session,
        member_email: &str,
        member_org_passphrase: Option<&KeySecret>,
        pending: &LtAuthDevice,
        typed_code: &str,
    ) -> Result<(), AdminDeviceApprovalError> {
        let members = admin_session
            .send_lt(LtCoreGetMembersReq::default())
            .await?;
        let member = find_member_by_email(&members, member_email)?;

        let member_id = member.id.clone();
        let member_keys = member.keys.0.clone();

        let activation_token =
            pending
                .activation_token
                .as_ref()
                .ok_or(AdminDeviceApprovalError::MissingField {
                    field: "activation_token",
                })?;
        let activation_address_id = pending.activation_address_id.as_ref().ok_or(
            AdminDeviceApprovalError::MissingField {
                field: "activation_address_id",
            },
        )?;

        let addrs = fetch_member_addresses_paginated(admin_session, &member_id).await?;
        let approval_keys =
            unlock_member_approval_keys(self, &addrs, &member_keys, member_org_passphrase)?;
        let decrypt_keys =
            decryption_keys_for_activation(&approval_keys, &addrs, activation_address_id)?;
        let device_secret =
            device_secret_from_activation(&self.pgp, &decrypt_keys, activation_token, typed_code)?;

        let reset_req = self
            .build_devices_reset_request(
                member_salts_session,
                &member_id,
                &member_keys,
                &approval_keys,
                pending,
                &device_secret,
            )
            .await?;

        admin_session.send_lt(reset_req).await?;
        Ok(())
    }

    async fn build_devices_reset_request(
        &self,
        member_salts_session: &Session,
        member_id: &LtCoreMemberEncId,
        member_keys: &UserKeys,
        approval_keys: &MemberApprovalKeys<P>,
        pending: &LtAuthDevice,
        device_secret: &DeviceSecret,
    ) -> Result<LtCorePostMembersDevicesResetReq, AdminDeviceApprovalError> {
        let member_user = member_salts_session.send_lt(LtCoreGetUsersReq).await?.user;
        let member_primary_id = primary_key_id(&member_user)?;
        let salts = member_salts_session.send_lt(LtCoreGetKeySaltsReq).await?;
        let new_password = super::super::random_password();
        let new_passphrase = derive_key_passphrase(
            &salts.key_salts,
            &member_primary_id,
            new_password.as_bytes(),
        )
        .map_err(AdminDeviceApprovalError::KeyPassphrase)?;

        let reset_user_keys = rearmor_user_keys(
            self,
            member_keys.as_ref(),
            &approval_keys.user_keys,
            &new_passphrase,
        )?;

        let encrypted_secret = device_secret
            .encrypt_passphrase(new_passphrase.as_ref())
            .map_err(AdminDeviceApprovalError::Crypto)?;

        Ok(LtCorePostMembersDevicesResetReq {
            member_id: member_id.clone(),
            body: LtCorePostMembersDevicesResetBody {
                auth_device_id: LtCoreAuthDeviceId(pending.id.clone()),
                encrypted_secret: Sensitive::new(encrypted_secret),
                user_keys: reset_user_keys,
            },
        })
    }
}
