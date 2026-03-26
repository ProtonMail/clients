use crate::services::proton::prelude::*;
use serde::Serialize;
use serde_with::{BoolFromInt, serde_as};

pub use mail_api_feature_flags::GetUnleashFeaturesRequest;

pub use contacts_api::{GetContactsEmailsOptions, GetContactsOptions, PutDeleteContacts};
pub use mail_api_labels::{
    GetLabelsByIdsOptions, GetLabelsOptions, PatchLabelRequest, PostLabelsRequest, PutLabelRequest,
};

pub use mail_account_api::protocol::proton::{GetCaptchaOptions, GetKeysAllOptions};
pub use mail_api_bug_report::PostReportBug;
pub use mail_api_device::RegisterDeviceRequest;
pub use mail_api_feature_flags::{
    GetLegacyFeatureFlagsOptions, MAX_LEGACY_FEATURES_PER_PAGE, PutFeatureFlagOverride,
};

/// Parameters for getting an event.
#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetEventOptions {
    /// TODO: Document this field.
    #[serde_as(as = "BoolFromInt")]
    pub conversation_counts: bool,

    /// TODO: Document this field.
    #[serde_as(as = "BoolFromInt")]
    pub message_counts: bool,
}

impl GetEventOptions {
    /// Return an instance of `GetEventOptions` with all counts set to `true`.
    #[must_use]
    pub fn all() -> Self {
        Self {
            conversation_counts: true,
            message_counts: true,
        }
    }

    /// Return an instance of `GetEventOptions` with all counts set to `false`.
    #[must_use]
    pub fn no_counts() -> Self {
        Self {
            conversation_counts: false,
            message_counts: false,
        }
    }
}

/// Parameters for getting images logo.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct GetImagesLogoOptions {
    /// The percent encoded address. Either Domain or Address are required.
    pub address: Option<PrivateEmail>,

    /// The bimi-selector of the message
    pub bimi_selector: Option<String>,

    /// Domain to get the logo for. Either Domain or Address are required.
    pub domain: Option<String>,

    /// Expected format for the image
    pub format: Option<String>,

    /// The maximum factor an image can be scaled up.
    pub max_scale_up_factor: Option<u8>,

    /// The theme being used.
    pub mode: Option<LightOrDarkMode>,

    /// The size of the logo to be returned.
    pub size: Option<u32>,
}
