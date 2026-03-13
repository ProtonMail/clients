pub mod delete_auth_devices;
pub mod get_auth_devices;
pub mod post_auth_devices;
pub mod post_auth_devices_associate;
pub mod post_auth_devices_device_id;

use num_enum::{IntoPrimitive, TryFromPrimitive};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
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
#[cfg_attr(feature = "facet", derive(facet::Facet))]
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

#[cfg_attr(feature = "facet", derive(facet::Facet))]
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
    pub create_time: i64,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub activate_time: Option<i64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub reject_time: Option<i64>,
    pub last_activity_time: i64,
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
