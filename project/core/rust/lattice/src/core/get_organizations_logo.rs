use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtRawBody, LtSlimAPIJSON};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
