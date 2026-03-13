use derive_more::Display;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
#[display("MissingScopes: {missing_scopes:?}")]
pub struct AccessTokenWithInsufficientScopeErrorDetails {
    pub missing_scopes: Vec<String>,
}
