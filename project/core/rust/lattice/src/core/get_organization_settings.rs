use std::borrow::Cow;

use crate::{AuthReq, LatticeError, LtContract, LtSlimAPIJSON};

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCoreGetOrganizationsSettingsReq;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreGetOrganizationsSettingsRes {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub show_name: Option<bool>,

    #[cfg_attr(feature = "serde", serde(rename = "LogoID"))]
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub logo_id: Option<String>,

    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub show_scribe_writing_assistant: Option<bool>,
}

impl LtContract for LtCoreGetOrganizationsSettingsReq {
    type Response = LtSlimAPIJSON<LtCoreGetOrganizationsSettingsRes>;
    type Body<'a> = LtSlimAPIJSON<()>;

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/organizations/settings"))
    }
}

impl AuthReq for LtCoreGetOrganizationsSettingsReq {}
