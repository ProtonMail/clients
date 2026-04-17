//! PHP Account/Core enum mirrors. Paths point to the canonical backend definitions.

use num_enum::{IntoPrimitive, TryFromPrimitive};

/// Definition: `bundles/AccountBundle/src/Organization/MemberState.php` (`MemberState` backed by int).
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(into = "i32", try_from = "i32"))]
pub enum LtCoreMemberState {
    Disabled = 0,
    Enabled = 1,
    Invited = 2,
}

/// Definition: `bundles/AccountBundle/src/Organization/MemberOrgKeyStatus.php` (`MemberOrgKeyStatus`).
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(into = "i32", try_from = "i32"))]
pub enum LtCoreMemberOrgKeyStatus {
    NoKey = 0,
    Active = 1,
    Missing = 2,
    Pending = 3,
}

/// Definition: `bundles/AccountBundle/src/Domain/DomainVerifyState.php` (`DomainVerifyState`).
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(into = "i32", try_from = "i32"))]
pub enum LtCoreDomainVerifyState {
    Default = 0,
    Exists = 1,
    Good = 2,
}

/// Definition: `apps/Account/app/Enum/SsoType.php` (`SsoType`).
#[repr(i32)]
#[derive(IntoPrimitive, TryFromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(into = "i32", try_from = "i32"))]
pub enum LtCoreSsoType {
    Default = 1,
    Edugain = 2,
}
