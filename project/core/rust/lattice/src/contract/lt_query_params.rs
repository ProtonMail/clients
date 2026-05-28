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
