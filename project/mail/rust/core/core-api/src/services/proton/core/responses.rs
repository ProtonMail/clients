//! Response structures for the Proton Core API.
//!
//! This module provides structures that are used to receive responses from the
//! Proton API. These structures are used to define the response bodies that are
//! received from the API when making a request.
//!
//! The purpose of the API service is to provide not only the means to make
//! requests, but also a formalisation of the data that is sent and received. To
//! this end, the structs in this module should mirror the API endpoint response
//! definitions, and NOT have any business logic or other functionality.
//!
//! To be clear, they should only contain data, and not methods; should not be
//! saved in the database; and should not be used for anything except providing
//! an interface for incoming data.
//!
//! Structs in this module should only implement [`Deserialize`], and should not
//! implement [`Serialize`](serde::Serialize). If anything in this module
//! implements [`Serialize`](serde::Serialize), it is a sign that a mistake has
//! been made. The exception here is for testing purposes, e.g. when mocking
//! response data — in which case implementing [`Serialize`](serde::Serialize)
//! conditionally, only in test mode, is advised.
//!
//! Any types that are children of the primary response structures should be
//! defined separately in the [`response_data`](crate::services::proton::response_data)
//! module, or in the [`common`](crate::services::proton::common) module if they
//! are used by both requests and responses.
//!

use proton_crypto_account::keys::{
    APIPublicAddressKeyGroup as PublicAddressKeyGroup,
    APIUnverifiedPublicAddressKeyGroup as UnverifiedPublicAddressKeyGroup, ArmoredPrivateKey,
    KeyId,
};
use serde::Deserialize;
use serde_with::{BoolFromInt, serde_as};

#[cfg(feature = "mocks")]
use serde::Serialize;

/// The response code indicating the status of the request.
/// A value of 1000 typically indicates success.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResponseCode(i32);

pub use mail_account_api::protocol::proton::{
    GetAddressResponse, GetAddressesResponse, GetKeysSaltsResponse, GetSettingsResponse,
    GetUsersResponse,
};

pub use contacts_api::{
    GetContactResponse, GetContactsEmailsResponse, GetContactsResponse, PutDeleteContactResponse,
    PutDeleteContactsResponse,
};
pub use mail_api_event_types::GetEventsLatestResponse;

/// Available public keys.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct GetKeysAllResponse {
    /// Information about the internal address itself, if it exists. Since the
    /// SKL is mandatory, this will never be nullable.
    #[serde(rename = "Address")]
    pub address_keys: PublicAddressKeyGroup,

    /// Information about the catch-all address itself, if it exists. This can
    /// be null if the address keys are valid
    #[serde(rename = "CatchAll")]
    pub catch_all_keys: Option<PublicAddressKeyGroup>,

    /// Tells whether this is an official Proton address.
    #[serde_as(as = "BoolFromInt")]
    pub is_proton: bool,

    /// True when domain has valid proton MX.
    #[serde(rename = "ProtonMX")]
    pub proton_mx: bool,

    /// Any other key that cannot be verified, such as Proton legacy keys or
    /// WKD.
    #[serde(rename = "Unverified")]
    pub unverified_keys: Option<UnverifiedPublicAddressKeyGroup>,

    /// List of warnings to show to the user related to phishing and message
    /// routing.
    pub warnings: Vec<String>,
}

pub use mail_api_labels::{
    GetLabelsResponse, PatchLabelResponse, PostLabelsResponse, PutLabelResponse,
};

/// Represents a user key in the response.
#[serde_as]
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserKey {
    /// Proton ID of the key.
    #[serde(rename = "ID")]
    pub id: KeyId,

    /// Proton version of the key.
    pub version: u32,

    /// `OpenPGP` private key armored.
    pub private_key: ArmoredPrivateKey,
    pub fingerprint: String,

    /// Is the key the primary key to use.
    #[serde_as(as = "BoolFromInt")]
    pub primary: bool,

    /// The key is active and should be decryptable.
    #[serde_as(as = "BoolFromInt")]
    pub active: bool,

    /// Secret for key recovery of a local file.
    pub recovery_secret: String,

    /// Signature for the recovery secret.
    pub recovery_secret_signature: String,

    /// Signature for the recovery secret.
    #[serde(default)]
    pub flags: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetProxyImageResponse {
    pub image: Vec<u8>,
    pub content_type: Option<String>,
    pub tracker_provider: Option<String>,
}

//  TRAITS
//==============================================================================

/// Marker trait for individual event responses.
pub trait GetEventResponse: Send + Sync {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::proton::core::common::LabelType;

    #[test]
    fn test_deserialize_labels_from_array() {
        let json = r##"{
            "Code": 1000,
            "Labels": [
                {
                    "ID": "sRNM_8TWzD4nSi55oC2B0-iV6avsMAAfDQZh7Bzsjy8c9Ip_c5OK5Tp5jB3mIEFmfUh3vFC9tevpCyXwoAa81w==",
                    "Name": "new 3",
                    "Path": "new 3",
                    "Type": 3,
                    "Color": "#415DF0",
                    "Order": 263,
                    "Notify": 1,
                    "Expanded": 0,
                    "Sticky": 0,
                    "Display": 1
                }
            ]
        }"##;

        let response: GetLabelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.labels.len(), 1);
        assert_eq!(
            response.labels[0].id.as_str(),
            "sRNM_8TWzD4nSi55oC2B0-iV6avsMAAfDQZh7Bzsjy8c9Ip_c5OK5Tp5jB3mIEFmfUh3vFC9tevpCyXwoAa81w=="
        );
        assert_eq!(response.labels[0].name, "new 3");
        assert_eq!(response.labels[0].path, Some("new 3".to_string()));
        assert_eq!(response.labels[0].label_type, LabelType::Folder);
        assert_eq!(response.labels[0].color, "#415DF0");
        assert_eq!(response.labels[0].order, 263);
    }

    #[test]
    fn test_deserialize_labels_from_map() {
        let json = r##"{
            "Code": 1000,
            "Labels": {
                "sRNM_8TWzD4nSi55oC2B0-iV6avsMAAfDQZh7Bzsjy8c9Ip_c5OK5Tp5jB3mIEFmfUh3vFC9tevpCyXwoAa81w==": {
                    "ID": "sRNM_8TWzD4nSi55oC2B0-iV6avsMAAfDQZh7Bzsjy8c9Ip_c5OK5Tp5jB3mIEFmfUh3vFC9tevpCyXwoAa81w==",
                    "Name": "new 3",
                    "Path": "new 3",
                    "Type": 3,
                    "Color": "#415DF0",
                    "Order": 263,
                    "Notify": 1,
                    "Expanded": 0,
                    "Sticky": 0,
                    "Display": 1
                }
            }
        }"##;

        let response: GetLabelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.labels.len(), 1);
        assert_eq!(
            response.labels[0].id.as_str(),
            "sRNM_8TWzD4nSi55oC2B0-iV6avsMAAfDQZh7Bzsjy8c9Ip_c5OK5Tp5jB3mIEFmfUh3vFC9tevpCyXwoAa81w=="
        );
        assert_eq!(response.labels[0].name, "new 3");
        assert_eq!(response.labels[0].path, Some("new 3".to_string()));
        assert_eq!(response.labels[0].label_type, LabelType::Folder);
        assert_eq!(response.labels[0].color, "#415DF0");
        assert_eq!(response.labels[0].order, 263);
    }
}
