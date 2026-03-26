pub use contacts_api::{
    ContactBasic, ContactCard, ContactEmail, ContactEmailEvent, ContactEvent, ContactEventV6,
    ContactFull, ContactLabelEventV6, ContactRootEventV6, ContactSendingPreferences,
};
pub use mail_api_event_types::Action;
pub use mail_api_feature_flags::{
    GetLegacyFeaturesResponse, GetUnleashFeaturesResponse, LegacyFeatureFlag, LegacyFeatureFlagId,
    LegacyFeatureFlagMetadata, LegacyFeatureFlagType, LegacyFeatureFlagVariant,
    PutFeatureFlagOverrideResponse, RangedValue, UnleashToggle, UnleashTogglePayload,
    UnleashTogglePayloadType, UnleashToggleVariant, Value,
};
pub use mail_api_labels::LabelEvent;

pub use mail_account_api::protocol::proton::{
    Address, AddressFlags, AddressSignedKeyList, AddressStatus, AddressType, DateFormat,
    DelinquentState, Density, Email, FidoKey, Flags, HighSecurity, LogAuth, Password, Phone,
    ProductUsedSpace, Referral, Role, SettingsFlags, TfaStatus, TimeFormat, TwoFa, User,
    UserMnemonicStatus, UserSettings, UserType, WeekStart,
};
pub use mail_api_session::auth::PasswordMode;
pub use mail_api_session::challenge::HumanVerificationChallenge;

use crate::services::proton::prelude::*;
use serde::Deserialize;
#[cfg(feature = "mocks")]
use serde::Serialize;
use serde_repr::Deserialize_repr;
#[cfg(feature = "mocks")]
use serde_repr::Serialize_repr;
use serde_with::{BoolFromInt, serde_as};

/// TODO: Document this enum.
#[derive(Clone, Copy, Debug, Deserialize_repr, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize_repr))]
#[repr(u8)]
pub enum EarlyAccess {
    /// TODO: Document this variant.
    Regular = 0,

    /// TODO: Document this variant.
    Beta = 1,
}

/// Data for an event related to an [`Address`] record.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct AddressEvent {
    #[serde(rename = "ID")]
    pub id: AddressId,
    pub action: Action,
    pub address: Option<Address>,
}

/// Core event data structure that matches the core fields from events.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct CoreEvent {
    #[serde(rename = "EventID")]
    pub event_id: EventId,
    pub addresses: Option<Vec<AddressEvent>>,
    pub labels: Option<Vec<LabelEvent>>,
    pub product_used_space: Option<ProductUsedSpace>,
    pub used_space: Option<i64>,
    pub user: Option<User>,
    pub user_settings: Option<UserSettings>,
    pub contacts: Option<Vec<ContactEvent>>,
    /// Indicates whether to refresh.
    pub refresh: u8,
    /// Whether we need to request more events after this.
    #[serde(rename = "More")]
    #[serde_as(as = "BoolFromInt")]
    pub has_more: bool,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct CoreEventV6 {
    pub users: Option<Vec<UserEventV6>>,
    pub addresses: Option<Vec<AddressEventV6>>,
    pub user_settings: Option<Vec<UserSettingsEventV6>>,
    pub refresh: bool,
    /// Whether we need to request more events after this.
    #[serde(rename = "More")]
    pub has_more: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct UserEventV6 {
    #[serde(rename = "ID")]
    pub id: UserId,
    pub action: Action,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct AddressEventV6 {
    #[serde(rename = "ID")]
    pub id: AddressId,
    pub action: Action,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct UserSettingsEventV6 {
    #[serde(rename = "ID")]
    pub id: AddressId,
    pub action: Action,
}
