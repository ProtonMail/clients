//! Common types used by the Proton Core API.

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::declare_proton_id;
pub use mail_api_event_types::{Action, EventId};
pub use mail_api_labels::{Label, LabelEvent, LabelId, LabelType};
pub use mail_proton_ids::ProtonIdMarker;

//  ENUMS
//==============================================================================

/// Human verification type returned by the API.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "lowercase")]
pub enum HumanVerificationType {
    Captcha,
    Email,
    Sms,
}

impl HumanVerificationType {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Captcha => "captcha",
            Self::Email => "email",
            Self::Sms => "sms",
        }
    }
}

/// The theme being used in Images Logo.
#[derive(Clone, Copy, Debug, Serialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LightOrDarkMode {
    Light,
    Dark,
}

/// In which environment are we going to register the device for push notifications.
#[derive(Clone, Copy, Debug, Deserialize_repr, Eq, Hash, PartialEq, Serialize_repr)]
#[repr(u8)]
pub enum DeviceEnvironment {
    Google = 4,
    AppleProd = 6,
    AppleBeta = 7,
    AppleProdET = 14,
    AppleDevET = 15,
    AppleDev = 16,
}

pub use mail_api_session::ids::{SessionId, UserId};

declare_proton_id! {
    pub AddressId
}
pub use contacts_api::{ContactEmailId, ContactId, ContactUID};
declare_proton_id! {
    pub SaltId
}
declare_proton_id! {
    pub IncomingDefaultId
}
