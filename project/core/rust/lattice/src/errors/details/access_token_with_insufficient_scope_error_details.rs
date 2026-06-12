use derive_more::Display;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
#[display("MissingScopes: {missing_scopes:?}")]
pub struct AccessTokenWithInsufficientScopeErrorDetails {
    pub missing_scopes: Vec<String>,
}
