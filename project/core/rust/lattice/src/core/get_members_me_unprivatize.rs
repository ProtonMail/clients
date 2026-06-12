use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON};

use super::unpriv_types::{
    LtCoreUnprivInvitationData, LtCoreUnprivInvitationSignature,
    LtCoreUnprivOrgKeyFingerprintSignature, LtCoreUnprivPgpPublicKey, LtCoreUnprivState,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LtCoreGetMembersMeUnprivatizeReq;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreGetMembersMeUnprivatizeRes {
    pub state: LtCoreUnprivState,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitation_data: Option<LtCoreUnprivInvitationData>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitation_signature: Option<LtCoreUnprivInvitationSignature>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitation_email: Option<String>,

    pub admin_email: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_key_fingerprint_signature: Option<LtCoreUnprivOrgKeyFingerprintSignature>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_public_key: Option<LtCoreUnprivPgpPublicKey>,

    pub private_intent: bool,
}

impl LtContract for LtCoreGetMembersMeUnprivatizeReq {
    type Response = LtSlimAPIJSON<LtCoreGetMembersMeUnprivatizeRes>;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/members/me/unprivatize"))
    }
}

impl AuthReq for LtCoreGetMembersMeUnprivatizeReq {}
