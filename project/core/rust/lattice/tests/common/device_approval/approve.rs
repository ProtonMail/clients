use core_key::SharedCryptoError;
use lattice::auth::devices::LtAuthDevice;
use lattice::core::members::devices::LtCorePostMembersDevicesResetReq;
use proton_crypto::crypto::PGPProviderSync;
use proton_crypto_account::salts::KeySecret;

use super::super::Session;
use super::super::unprivatize_admin::AdminPgpState;
use super::admin_device_approval_error::AdminDeviceApprovalError;

pub async fn approve_member_device<P: PGPProviderSync>(
    admin_state: &AdminPgpState<P>,
    admin_session: &Session,
    member_email: &str,
    member_org_passphrase: Option<&KeySecret>,
    pending: &LtAuthDevice,
    typed_code: &str,
) -> Result<(), AdminDeviceApprovalError> {
    let member = admin_session
        .find_member_by_email(member_email)
        .await
        .map_err(AdminDeviceApprovalError::Org)?;

    let addrs = admin_session
        .member_addresses(&member.id)
        .await
        .map_err(AdminDeviceApprovalError::Transport)?;

    let body = admin_state
        .org_admin_pgp()
        .build_devices_reset_for_pending(
            &member.keys.0,
            &addrs,
            member_org_passphrase,
            pending,
            typed_code,
        )
        .map_err(|e| match e {
            SharedCryptoError::PendingAuthDeviceMissingField { field } => {
                AdminDeviceApprovalError::MissingField { field }
            }
            e => AdminDeviceApprovalError::Crypto(e),
        })?;

    admin_session
        .send_lt(LtCorePostMembersDevicesResetReq {
            member_id: member.id,
            body,
        })
        .await?;
    Ok(())
}
