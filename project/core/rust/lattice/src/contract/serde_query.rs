use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;

use crate::contract::LtRequestQueryParams;
use crate::{LatticeError, Sensitive};

pub struct LtSerdeQueryParams<T: Serialize>(pub T);

impl<T: Serialize> LtRequestQueryParams for LtSerdeQueryParams<T> {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        Ok(
            url::form_urlencoded::parse(serde_qs::to_string(&self.0)?.as_bytes())
                .into_owned()
                .map(|(k, v)| (Cow::Owned(k), Sensitive::new(v)))
                .collect(),
        )
    }
}
