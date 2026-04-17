use std::{borrow::Cow, collections::HashMap};

use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON};

use super::{LtCoreDomainId, post_domains::LtCoreDomainOutput};

/// Request to get a specific domain and its DNS check
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct LtCoreGetDomainReq {
    /// The domain ID (enc_id in path)
    #[cfg_attr(feature = "serde", serde(skip))]
    pub domain_id: LtCoreDomainId,

    /// When `Some`, requests a DNS verification refresh (`true` → query parameter `Refresh=1`, `false` → `Refresh=0`).
    /// When `None`, the parameter is omitted and the API uses its default behaviour.
    pub refresh: Option<bool>,
}

/// Response from the get domain endpoint
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "PascalCase", deny_unknown_fields)
)]
pub struct LtCoreGetDomainRes {
    /// The domain details
    pub domain: LtCoreDomainOutput,
}

impl LtContract for LtCoreGetDomainReq {
    type Response = LtSlimAPIJSON<LtCoreGetDomainRes>;
    type Body<'a> = LtSlimAPIJSON<()>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!("/core/v4/domains/{}", self.domain_id)))
    }

    fn query(&self) -> Result<Option<HashMap<String, String>>, LatticeError> {
        if let Some(refresh) = self.refresh {
            Ok(Some(HashMap::from([(
                String::from("Refresh"),
                String::from(if refresh { "1" } else { "0" }),
            )])))
        } else {
            Ok(None)
        }
    }
}

impl AuthReq for LtCoreGetDomainReq {}
