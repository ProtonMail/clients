use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

use super::{LtCoreDomainId, account_enums::LtCoreSsoType};

/// Request to set up SAML SSO configuration for a domain
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCorePostSamlSetupFieldsReq {
    /// The domain ID to configure SAML for
    #[cfg_attr(feature = "serde", serde(rename = "DomainID"))]
    pub domain_id: LtCoreDomainId,

    /// The SSO URL endpoint
    #[cfg_attr(feature = "serde", serde(rename = "SSOURL"))]
    pub sso_url: String,

    /// The SSO Entity ID (identifier)
    #[cfg_attr(feature = "serde", serde(rename = "SSOEntityID"))]
    pub sso_entity_id: String,

    /// The X.509 certificate in PEM format
    pub certificate: String,

    /// The SAML configuration type
    #[cfg_attr(feature = "serde", serde(rename = "Type"))]
    pub saml_type: LtCoreSsoType,
}

/// SSO configuration details returned by the API
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCoreSsoConfig {
    /// The SSO configuration ID
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: String,

    /// The SSO URL endpoint
    #[cfg_attr(feature = "serde", serde(rename = "SSOURL"))]
    pub sso_url: String,

    /// The SSO Entity ID (identifier)
    #[cfg_attr(feature = "serde", serde(rename = "SSOEntityID"))]
    pub sso_entity_id: String,

    /// The issuer ID
    #[cfg_attr(feature = "serde", serde(rename = "IssuerID"))]
    pub issuer_id: String,

    /// The X.509 certificate in PEM format
    pub certificate: String,

    /// The domain ID
    #[cfg_attr(feature = "serde", serde(rename = "DomainID"))]
    pub domain_id: LtCoreDomainId,

    /// The SCIM OAuth Client ID (optional)
    #[cfg_attr(feature = "serde", serde(rename = "SCIMOauthClientID"))]
    pub scim_oauth_client_id: Option<String>,

    /// The callback URL
    #[cfg_attr(feature = "serde", serde(rename = "CallbackURL"))]
    pub callback_url: String,

    /// The allowed domain
    pub allowed_domain: String,

    /// Send subject flag
    pub send_subject: i32,

    /// Whether SSO is enabled
    pub enabled: bool,

    /// The SAML configuration type
    #[cfg_attr(feature = "serde", serde(rename = "Type"))]
    pub saml_type: LtCoreSsoType,

    /// Edugain affiliations
    pub edugain_affiliations: Vec<String>,
}

/// Response from the SAML setup fields endpoint
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCorePostSamlSetupFieldsRes {
    /// The SSO configuration
    #[cfg_attr(feature = "serde", serde(rename = "SSO"))]
    pub sso: LtCoreSsoConfig,
}

impl LtContract for LtCorePostSamlSetupFieldsReq {
    type Response = LtSlimAPIJSON<LtCorePostSamlSetupFieldsRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/saml/setup/fields"))
    }
}

impl AuthReq for LtCorePostSamlSetupFieldsReq {}
