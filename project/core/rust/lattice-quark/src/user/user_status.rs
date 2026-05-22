use derive_more::Display;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Display,
    Clone,
    Copy,
    IntoPrimitive,
    TryFromPrimitive,
    Deserialize,
    Serialize
)]
#[serde(try_from = "u8", into = "u8")]
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
