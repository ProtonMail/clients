//! Common types used by the Proton Core API.

use serde::{Deserialize, Serialize};

pub use mail_api_event_types::{Action, EventId};
pub use mail_api_labels::{Label, LabelEvent, LabelId, LabelType};
pub use mail_proton_ids::ProtonIdMarker;

pub use mail_account_api::protocol::proton::{AddressId, SaltId};
pub use mail_api_device::DeviceEnvironment;
pub use mail_api_session::ids::{SessionId, UserId};

/// The theme being used in Images Logo.
#[derive(Clone, Copy, Debug, serde::Serialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LightOrDarkMode {
    Light,
    Dark,
}

pub use contacts_api::{ContactEmailId, ContactId, ContactUID};

use crate::declare_proton_id;
declare_proton_id! {
    pub IncomingDefaultId
}

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
