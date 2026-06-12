use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method};

use super::{LtCoreDomainId, account_enums::LtCoreDomainVerifyState};

/// Request to create a new domain
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCorePostDomainsReq {
    /// The domain name to be created
    pub name: String,

    /// True if this domain is intended for Mail usage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_for_mail: Option<bool>,

    /// True if this domain is intended for SSO usage
    #[serde(skip_serializing_if = "Option::is_none", rename = "AllowedForSSO")]
    pub allowed_for_sso: Option<bool>,
}

/// DKIM key details
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCoreDkimKey {
    #[serde(rename = "ID")]
    pub id: String,

    pub selector: String,

    pub public_key: String,

    pub algorithm: i32,

    pub state: i32,

    #[serde(rename = "DNSState")]
    pub dns_state: i32,

    pub create_time: i64,
}

/// DKIM configuration for a hostname
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCoreDkimConfig {
    pub hostname: String,

    #[serde(rename = "CNAME")]
    pub cname: Option<String>,

    pub key: Option<LtCoreDkimKey>,
}

/// DKIM settings for a domain
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCoreDkim {
    pub state: i32,

    pub config: Vec<LtCoreDkimConfig>,
}

/// Domain flags indicating usage intent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LtCoreDomainFlags {
    #[serde(rename = "mail-intent")]
    pub mail_intent: bool,

    #[serde(rename = "sso-intent")]
    pub sso_intent: bool,

    #[serde(rename = "dark-web-monitoring")]
    pub dark_web_monitoring: bool,
}

/// Domain information returned by the API
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCoreDomainOutput {
    #[serde(rename = "ID")]
    pub id: LtCoreDomainId,

    pub domain_name: String,

    #[serde(rename = "Type")]
    pub domain_type: i32,

    pub state: i32,

    pub last_active_time: i64,

    pub check_time: i64,

    pub warn_time: i64,

    pub verify_code: String,

    pub verify_state: LtCoreDomainVerifyState,

    pub mx_state: i32,

    pub spf_state: i32,

    pub dmarc_state: i32,

    #[serde(rename = "DKIM")]
    pub dkim: LtCoreDkim,

    pub flags: LtCoreDomainFlags,
}

/// Response from the create domain endpoint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct LtCorePostDomainsRes {
    /// The details of the newly created domain
    pub domain: LtCoreDomainOutput,
}

impl LtContract for LtCorePostDomainsReq {
    type Response = LtSlimAPIJSON<LtCorePostDomainsRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/domains"))
    }
}

impl AuthReq for LtCorePostDomainsReq {}
