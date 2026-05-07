use std::borrow::Cow;

use lattice::{LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

use crate::{CORE_V4, ContactGroupId};

pub struct DeleteContactGroupRequest {
    pub id: ContactGroupId,
}

impl LtContract for DeleteContactGroupRequest {
    type Response = LtSlimAPIJSON<()>;
    type Body<'b> = LtSlimAPIJSON<()>;
    type Query<'q> = LtNoQueryParams;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("{CORE_V4}/labels/{}", self.id)))
    }

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Delete)
    }
}

#[cfg(feature = "mocks")]
impl DeleteContactGroupRequest {
    pub fn mock(id: ContactGroupId) -> wiremock::MockBuilder {
        use wiremock::matchers::*;
        wiremock::Mock::given(method("DELETE")).and(path(format!("api{CORE_V4}/{id}")))
    }
}
