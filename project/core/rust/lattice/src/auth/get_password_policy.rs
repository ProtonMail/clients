use std::borrow::Cow;

use crate::{AuthReq, LatticeContract, LatticeError, UnauthReq};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtAuthGetPasswordPoliciesReq;
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthGetPasswordPoliciesRes {
    pub password_policies: Vec<LtAuthPasswordPolicyRes>,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtAuthPasswordPolicyRes {
    /// The name of the password policy. This serves as identifier.
    pub policy_name: String,

    /// The state of the password policy. Disabled policies are not returned.
    pub state: LtAuthPasswordPolicyState,

    /// The requirement message. This is a relatively short string informing the
    /// user how to fulfill the policy.
    pub requirement_message: String,

    /// The error message. This string is intended to be displayed to the user
    /// when they try to proceed with a password that does not respect the
    /// policy.
    pub error_message: String,

    /// The regex. It should be applied to the password. If it returns true, the
    /// policy passed.
    pub regex: String,

    /// Whether the policy should be hidden when the password respects it. In
    /// other words it should only appear when violated.
    pub hide_if_valid: bool,
}

#[repr(i32)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
#[cfg_attr(feature = "serde", serde(into = "i32", try_from = "i32"))]
pub enum LtAuthPasswordPolicyState {
    Disabled = 0,
    Enabled = 1,
    Optional = 2,
    Unknown = -1,
}

impl From<LtAuthPasswordPolicyState> for i32 {
    fn from(val: LtAuthPasswordPolicyState) -> Self {
        val as i32
    }
}

impl From<i32> for LtAuthPasswordPolicyState {
    fn from(value: i32) -> Self {
        match value {
            0 => LtAuthPasswordPolicyState::Disabled,
            1 => LtAuthPasswordPolicyState::Enabled,
            2 => LtAuthPasswordPolicyState::Optional,
            _ => LtAuthPasswordPolicyState::Unknown,
        }
    }
}

impl LatticeContract for LtAuthGetPasswordPoliciesReq {
    type Body<'a> = ();
    type Response = LtAuthGetPasswordPoliciesRes;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/auth/v4/password-policies"))
    }
}

impl UnauthReq for LtAuthGetPasswordPoliciesReq {}
impl AuthReq for LtAuthGetPasswordPoliciesReq {}
