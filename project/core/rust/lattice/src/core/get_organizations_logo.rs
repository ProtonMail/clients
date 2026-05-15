use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtRawBody, LtSlimAPIJSON};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetOrganizationsLogoReq {
    pub logo_id: String,
}

impl LtContract for LtCoreGetOrganizationsLogoReq {
    type Response = LtRawBody;
    type Body<'a> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/organizations/logo/{}",
            self.logo_id
        )))
    }
}

impl AuthReq for LtCoreGetOrganizationsLogoReq {}
