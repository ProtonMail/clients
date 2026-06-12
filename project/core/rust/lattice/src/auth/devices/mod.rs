pub mod delete_auth_devices;
pub mod get_auth_devices;
pub mod post_auth_devices_associate;
pub mod post_auth_devices_create;
pub mod post_auth_devices_device_id;
pub mod put_auth_devices_device_id_admin;
pub mod put_auth_devices_device_id_reject;
mod unix_timestamp;

pub use delete_auth_devices::{LtAuthDeleteDevicesReq, LtAuthDeleteDevicesRes};
pub use get_auth_devices::{LtAuthGetDevicesReq, LtAuthGetDevicesRes};
pub use post_auth_devices_associate::{
    LtAuthPostDevicesAssociateReq, LtAuthPostDevicesAssociateRes,
};
pub use post_auth_devices_create::{LtAuthPostDevicesCreateReq, LtAuthPostDevicesCreateRes};
pub use post_auth_devices_device_id::LtAuthPostDevicesDeviceIDReq;
pub use put_auth_devices_device_id_admin::LtAuthPutDevicesDeviceIDAdminReq;
pub use put_auth_devices_device_id_reject::LtAuthPutDevicesDeviceIDRejectReq;
pub use unix_timestamp::LtUnixTimestamp;

use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};

use crate::{
    auth::LtAuthAddressId,
    core::{LtCoreAuthDeviceId, LtCoreMemberEncId},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthAssociatedDevice {
    #[serde(rename = "ID")]
    pub id: String,
    pub encrypted_secret: String,
}

#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum LtAuthDeviceState {
    Inactive = 0,
    Active = 1,
    PendingActivation = 2,
    PendingAdminActivation = 3,
    Rejected = 4,
    NoSession = 5,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtAuthDevice {
    #[serde(rename = "ID")]
    pub id: LtCoreAuthDeviceId,
    pub state: LtAuthDeviceState,
    pub name: String,
    pub localized_client_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    pub create_time: LtUnixTimestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activate_time: Option<LtUnixTimestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reject_time: Option<LtUnixTimestamp>,
    pub last_activity_time: LtUnixTimestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ActivationAddressID")]
    pub activation_address_id: Option<LtAuthAddressId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "MemberID")]
    pub member_id: Option<LtCoreMemberEncId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_token: Option<String>,
}
