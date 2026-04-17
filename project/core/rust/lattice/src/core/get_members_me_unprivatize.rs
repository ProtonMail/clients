use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetMembersMeUnprivatizeReq;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetMembersMeUnprivatizeRes {
    pub state: LtCoreUnprivatizationState,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub invitation_data: Option<String>,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub invitation_signature: Option<String>,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub invitation_email: Option<String>,

    pub admin_email: String,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub org_key_fingerprint_signature: Option<String>,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub org_public_key: Option<String>,

    pub private_intent: bool,
}

#[repr(i32)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(into = "i32", try_from = "i32"))]
pub enum LtCoreUnprivatizationState {
    Declined = 0,
    Pending = 1,
    Ready = 2,
}

impl TryFrom<i32> for LtCoreUnprivatizationState {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Declined),
            1 => Ok(Self::Pending),
            2 => Ok(Self::Ready),
            _ => Err(format!("invalid unprivatization state: {value}")),
        }
    }
}

impl From<LtCoreUnprivatizationState> for i32 {
    fn from(val: LtCoreUnprivatizationState) -> Self {
        val as i32
    }
}

impl LtContract for LtCoreGetMembersMeUnprivatizeReq {
    type Response = LtSlimAPIJSON<LtCoreGetMembersMeUnprivatizeRes>;
    type Body<'a> = LtSlimAPIJSON<()>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/members/me/unprivatize"))
    }
}

impl AuthReq for LtCoreGetMembersMeUnprivatizeReq {}
