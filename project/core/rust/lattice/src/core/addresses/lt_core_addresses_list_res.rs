use crate::core::LtCoreAddress;

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
