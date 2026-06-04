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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthAssociatedDevice {
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: String,
    pub encrypted_secret: String,
}

#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(into = "i32", try_from = "i32"))]
pub enum LtAuthDeviceState {
    Inactive = 0,
    Active = 1,
    PendingActivation = 2,
    PendingAdminActivation = 3,
    Rejected = 4,
    NoSession = 5,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthDevice {
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: String,
    pub state: LtAuthDeviceState,
    pub name: String,
    pub localized_client_name: String,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub platform: Option<String>,
    pub create_time: LtUnixTimestamp,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub activate_time: Option<LtUnixTimestamp>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub reject_time: Option<LtUnixTimestamp>,
    pub last_activity_time: LtUnixTimestamp,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub activation_token: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "serde", serde(rename = "ActivationAddressID"))]
    pub activation_address_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "serde", serde(rename = "MemberID"))]
    pub member_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub device_token: Option<String>,
}
