use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{
    LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive, UnauthReq,
};

pub const DATA_V1_METRICS_PATH: &str = "/data/v1/metrics";

/// Value for the `Priority` header (background, low priority).
pub const METRICS_PRIORITY_HEADER_VALUE: &str = "u=6";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtDataPostMetricsReq {
    pub metrics: Vec<LtDataPostMetricsElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtDataPostMetricsElement {
    pub name: String,
    pub version: u64,
    pub timestamp: i64,
    pub data: serde_json::Value,
}

impl LtContract for LtDataPostMetricsReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'b> = LtSlimAPIJSON<&'b Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed(DATA_V1_METRICS_PATH))
    }

    fn headers(
        &self,
    ) -> Result<std::collections::HashMap<String, Sensitive<String>>, LatticeError> {
        Ok(std::collections::HashMap::from([(
            "Priority".to_string(),
            Sensitive::new(METRICS_PRIORITY_HEADER_VALUE.to_string()),
        )]))
    }
}

impl UnauthReq for LtDataPostMetricsReq {}
