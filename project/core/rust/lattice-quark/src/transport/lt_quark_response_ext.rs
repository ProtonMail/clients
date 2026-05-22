use lattice::{LatticeError, transport::LtWireResponse};

use crate::{LtQuarkContract, LtQuarkRes};

/// Parse a lattice wire response as a Quark contract response.
pub trait LtQuarkResponseExt {
    fn into_quark_response<T: LtQuarkContract>(self) -> Result<T::Response, LatticeError>;
}

impl LtQuarkResponseExt for LtWireResponse {
    fn into_quark_response<T: LtQuarkContract>(self) -> Result<T::Response, LatticeError> {
        if self.status != 200 {
            return Err(LatticeError::UnexpectedStatusCode(
                self.status,
                self.body.into_inner(),
            ));
        }
        <T::Response as LtQuarkRes>::from_quark_body(&self.body)
    }
}
