use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON};

use super::unpriv_types::{
    LtCoreUnprivInvitationData, LtCoreUnprivInvitationSignature,
    LtCoreUnprivOrgKeyFingerprintSignature, LtCoreUnprivPgpPublicKey, LtCoreUnprivState,
};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetMembersMeUnprivatizeReq;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetMembersMeUnprivatizeRes {
    pub state: LtCoreUnprivState,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub invitation_data: Option<LtCoreUnprivInvitationData>,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub invitation_signature: Option<LtCoreUnprivInvitationSignature>,

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
    pub org_key_fingerprint_signature: Option<LtCoreUnprivOrgKeyFingerprintSignature>,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub org_public_key: Option<LtCoreUnprivPgpPublicKey>,

    pub private_intent: bool,
}

impl LtContract for LtCoreGetMembersMeUnprivatizeReq {
    type Response = LtSlimAPIJSON<LtCoreGetMembersMeUnprivatizeRes>;
    type Body<'a> = LtSlimAPIJSON<()>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/members/me/unprivatize"))
    }
}

impl AuthReq for LtCoreGetMembersMeUnprivatizeReq {}
