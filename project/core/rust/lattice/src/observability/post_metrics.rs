use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::{LatticeContract, LatticeError, Method, UnauthReq};

pub const DATA_V1_METRICS_PATH: &str = "/data/v1/metrics";

/// Value for the `Priority` header (background, low priority).
pub const METRICS_PRIORITY_HEADER_VALUE: &str = "u=6";

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
#[derive(Debug, Clone)]
pub struct LtDataPostMetricsReq {
    pub metrics: Vec<LtDataPostMetricsElement>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
#[derive(Debug, Clone)]
pub struct LtDataPostMetricsElement {
    pub name: String,
    pub version: u64,
    pub timestamp: i64,
    pub data: serde_json::Value,
}

impl LatticeContract for LtDataPostMetricsReq {
    type Response = ();
    type Body<'b> = &'b Self;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(self))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed(DATA_V1_METRICS_PATH))
    }

    fn headers(&self) -> Result<std::collections::HashMap<String, String>, LatticeError> {
        Ok(std::collections::HashMap::from([(
            "Priority".to_string(),
            METRICS_PRIORITY_HEADER_VALUE.to_string(),
        )]))
    }
}

impl UnauthReq for LtDataPostMetricsReq {}
