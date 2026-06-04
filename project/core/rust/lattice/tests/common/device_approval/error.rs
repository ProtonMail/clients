use derive_more::{Display, Error, From};
use lattice_muon2::LtTransportError;

use super::super::unprivatize_admin::UnprivatizeAdminError;
use super::admin_device_approval_error::AdminDeviceApprovalError;
use super::device_secret_error::DeviceSecretError;
use super::pending_device_error::PendingDeviceError;

#[derive(Debug, Display, Error, From)]
pub enum DeviceApprovalError {
    #[display("{_0}")]
    Transport(#[from] LtTransportError),
    #[display("{_0}")]
    Setup(#[from] UnprivatizeAdminError),
    #[display("{_0}")]
    Pending(#[from] PendingDeviceError),
    #[display("{_0}")]
    Admin(#[from] AdminDeviceApprovalError),
    #[display("{_0}")]
    Crypto(#[from] DeviceSecretError),
}
