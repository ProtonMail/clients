use std::collections::HashMap;

#[cfg(feature = "quark")]
use crate::quark::{LtQuarkContract, LtQuarkRes};
use crate::{LatticeError, LtApiResponseError, LtContract, LtResponseBody, Sensitive};

#[derive(Debug, Clone)]
pub struct LtWireResponse {
    pub status: u16,
    pub headers: HashMap<String, Sensitive<String>>,
    pub body: Sensitive<Vec<u8>>,
}

impl LtWireResponse {
    pub fn into_contract_response<T: LtContract>(self) -> Result<T::Response, LatticeError> {
        if (200..=304).contains(&self.status) {
            return T::Response::from_body(&self.body);
        }

        if (400..500).contains(&self.status) {
            let value: LtApiResponseError =
                serde_json::from_slice::<LtApiResponseError>(&self.body).map_err(|e| {
                    LatticeError::SerdeJSON(
                        e,
                        String::from_utf8(self.body.clone().into_inner()).ok(),
                    )
                })?;

            return Err(LatticeError::ApiError(self.status, Box::new(value)));
        }

        Err(LatticeError::UnexpectedStatusCode(
            self.status,
            self.body.into_inner(),
        ))
    }

    #[cfg(feature = "quark")]
    pub fn into_quark_response<T: LtQuarkContract>(self) -> Result<T::Response, LatticeError> {
        if self.status != 200 {
            return Err(LatticeError::UnexpectedStatusCode(
                self.status,
                self.body.into_inner(),
            ));
        }
        <T::Response as LtQuarkRes>::from_quark_body(&self.body)
    }
}
