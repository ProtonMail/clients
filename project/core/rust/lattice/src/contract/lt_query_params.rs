use std::borrow::Cow;
use std::collections::HashMap;

use crate::{LatticeError, Sensitive};

pub trait LtRequestQueryParams {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError>;
}

impl<T: LtRequestQueryParams> LtRequestQueryParams for &T {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        T::to_query_params(self)
    }
}

pub struct LtNoQueryParams;

impl LtRequestQueryParams for LtNoQueryParams {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        Ok(HashMap::new())
    }
}

#[cfg(feature = "serde_qs")]
pub struct LtSerdeQueryParams<T: serde::Serialize>(T);

#[cfg(feature = "serde_qs")]
impl<T: serde::Serialize> LtSerdeQueryParams<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }
}

#[cfg(feature = "serde_qs")]
impl<T: serde::Serialize> LtRequestQueryParams for LtSerdeQueryParams<T> {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        let headers = url::form_urlencoded::parse(serde_qs::to_string(&self.0)?.as_bytes())
            .into_owned()
            .map(|(k, v)| (Cow::Owned(k), Sensitive::new(v)))
            .collect::<HashMap<_, _>>();
        Ok(headers)
    }
}
