use std::collections::HashMap;

#[cfg(feature = "quark")]
use crate::quark::LtQuarkContract;
use crate::{LatticeError, LtContract, LtRequestQueryParams, Sensitive};

use super::wire_method::LtWireMethod;

#[derive(Debug, Clone)]
pub struct LtWireRequest {
    pub headers: HashMap<String, Sensitive<String>>,
    pub method: LtWireMethod,
    pub path: String,
    pub query: Vec<(String, Sensitive<String>)>,
}

impl LtWireRequest {
    pub fn from_contract<T: LtContract>(contract: &T) -> Result<Self, LatticeError> {
        let method = contract.method()?;
        let wire_method = LtWireMethod::from_contract_method(method)?;
        let path = contract.path()?.into_owned();

        let query = if let Some(q) = contract.query() {
            q.to_query_params()?
                .into_iter()
                .map(|(k, v)| (k.into_owned(), v))
                .collect()
        } else {
            Vec::new()
        };

        let headers = contract.headers()?;

        Ok(Self {
            headers,
            method: wire_method,
            path,
            query,
        })
    }

    #[cfg(feature = "quark")]
    pub fn from_contract_quark<T: LtQuarkContract>(contract: &T) -> Result<Self, LatticeError> {
        let url = format!("/internal/quark/raw::{}", T::COMMAND_PATH);
        let params = contract.params()?.as_command();
        Ok(Self {
            headers: HashMap::new(),
            method: LtWireMethod::Get,
            path: url,
            query: vec![("strInput".to_string(), Sensitive::new(params))],
        })
    }
}
