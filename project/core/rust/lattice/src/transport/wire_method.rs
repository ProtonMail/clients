use crate::{LatticeError, LtRequestBody, Method, Sensitive};

#[derive(Debug, Clone)]
pub enum LtWireMethod {
    Get,
    Post(Sensitive<Vec<u8>>),
    Put(Sensitive<Vec<u8>>),
    Delete,
}

impl LtWireMethod {
    pub fn from_contract_method<T: LtRequestBody>(method: Method<T>) -> Result<Self, LatticeError> {
        match method {
            Method::Get => Ok(Self::Get),
            Method::Post(body) => Ok(Self::Post(Sensitive::new(body.to_body()?))),
            Method::Put(body) => Ok(Self::Put(Sensitive::new(body.to_body()?))),
            Method::Delete => Ok(Self::Delete),
        }
    }
}
