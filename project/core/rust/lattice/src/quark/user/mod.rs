use derive_more::Display;
use num_enum::{IntoPrimitive, TryFromPrimitive};

pub mod domain_create;
pub mod organization_create;
pub mod subuser_create;
pub mod user_create;
pub mod user_reset;

#[derive(Debug, Display, Clone, Copy, IntoPrimitive, TryFromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(try_from = "u8", into = "u8"))]
#[display("{}", self.to_owned() as u8)]
#[repr(u8)]
pub enum LtQuarkUserStatus {
    Deleted = 0,
    Disabled = 1,
    Active = 2,
    VPNAdmin = 3,
    Admin = 4,
    Super = 5,
}

#[derive(Debug, Display, Clone, Copy)]
pub enum LtQuarkKeyType {
    Curve25519,
    RSA2048,
    RSA4096,
}
