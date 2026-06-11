use crate::LtSlimApiPresenceQuery;
use crate::core::LtCoreAddress;

/// Query parameter shared by address list endpoints (optional `Handles` flag).
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreAddressesListQuery {
    /// Presence-only SlimAPI query flag (wire value is empty), e.g. `Handles`.
    #[cfg_attr(
        feature = "serde",
        serde(rename = "Handles", skip_serializing_if = "Option::is_none")
    )]
    pub handles: Option<LtSlimApiPresenceQuery>,
}

/// Response body fields beside `Code` for address list endpoints.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreAddressesListRes {
    pub addresses: Vec<LtCoreAddress>,
    /// This will only be present if the request includes pagination.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub total: Option<u32>,
    // TODO: `SignedAddressList` on `GET /core/v4/addresses` is `[]` or `{ Data, Signature }`.
    // pub signed_address_list: Option<LtCoreSignedAddressList>,
}
