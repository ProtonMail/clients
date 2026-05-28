use crate::{LtSlimApiPageQuery, LtSlimApiPresenceQuery};

pub const ADDRESSES_LIST_MAX_PAGE_SIZE: u32 = 150;

/// Query parameters shared by address list endpoints (`Page` / `PageSize`, optional `Handles`).
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreAddressesListQuery {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub pagination: LtSlimApiPageQuery<ADDRESSES_LIST_MAX_PAGE_SIZE>,
    /// Presence-only SlimAPI query flag (wire value is empty), e.g. `Handles`.
    #[cfg_attr(
        feature = "serde",
        serde(rename = "Handles", skip_serializing_if = "Option::is_none")
    )]
    pub handles: Option<LtSlimApiPresenceQuery>,
}
