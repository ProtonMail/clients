use mail_api_shared::ApiServiceResult;
use mail_api_utils::{PaginateOptions, PaginateResponse};
use mail_muon::common::Sender;
use mail_muon::http::HttpReqExt;
use mail_muon::{GET, POST, PUT, ProtonRequest, ProtonResponse, serde_to_query};
use serde::{Deserialize, Serialize};
use serde_with::{StringWithSeparator, formats::CommaSeparator, serde_as};
use smart_default::SmartDefault;

pub use core_feature_flags::{
    GetUnleashFeaturesContext, GetUnleashFeaturesRequest, GetUnleashFeaturesResponse, UNLEASH_V2,
    UnleashToggle, UnleashTogglePayload, UnleashTogglePayloadType, UnleashToggleVariant,
};

const CORE_V4: &str = "/core/v4";

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct GetLegacyFeaturesResponse {
    pub total: u64,
    pub features: Vec<LegacyFeatureFlag>,
}

impl PaginateResponse<LegacyFeatureFlag> for GetLegacyFeaturesResponse {
    fn total(&self) -> u64 {
        self.total
    }

    fn items(self) -> Vec<LegacyFeatureFlag> {
        self.features
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct PutFeatureFlagOverrideResponse {
    pub feature: LegacyFeatureFlag,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct LegacyFeatureFlag {
    #[serde(flatten)]
    pub metadata: LegacyFeatureFlagMetadata,
    #[serde(flatten)]
    pub variant: LegacyFeatureFlagVariant,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct LegacyFeatureFlagMetadata {
    pub code: String,
    #[serde(default)]
    pub global: bool,
    #[serde(default)]
    pub writable: bool,
    #[serde(default)]
    pub expiration_time: Option<u64>,
    #[serde(default)]
    pub update_time: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
#[must_use]
pub struct Value<T> {
    pub value: T,
    pub default_value: T,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
#[must_use]
pub struct RangedValue<T> {
    pub value: T,
    pub default_value: T,
    pub minimum: T,
    pub maximum: T,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "Type")]
pub enum LegacyFeatureFlagVariant {
    Boolean(Value<bool>),
    Integer(RangedValue<i32>),
    Float(RangedValue<f64>),
    String(Value<String>),
    Enumeration(Value<String>),
    Mixed(Value<serde_json::Value>),
}

impl LegacyFeatureFlagVariant {
    #[must_use]
    pub fn into_bool(self) -> Option<Value<bool>> {
        match self {
            Self::Boolean(b) => Some(b),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LegacyFeatureFlagType {
    Boolean,
    Integer,
    Float,
    String,
    Enumeration,
    Mixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LegacyFeatureFlagId {
    RatingBoosterAndroid,
    RatingBoosterIOS,
}

impl std::fmt::Display for LegacyFeatureFlagId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RatingBoosterAndroid => write!(f, "RatingAndroidMail"),
            Self::RatingBoosterIOS => write!(f, "RatingIOSMail"),
        }
    }
}

impl LegacyFeatureFlagId {
    #[must_use]
    pub fn default_filter() -> Vec<Self> {
        #[cfg(target_os = "android")]
        return vec![Self::RatingBoosterAndroid];

        #[cfg(target_os = "ios")]
        return vec![Self::RatingBoosterIOS];

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        vec![]
    }
}

/// Maximum page size supported by the API.
pub const MAX_LEGACY_FEATURES_PER_PAGE: u64 = 150;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Options for fetching legacy feature flags.
#[serde_as]
#[derive(Clone, Debug, Serialize, SmartDefault)]
#[serde(rename_all = "PascalCase")]
pub struct GetLegacyFeatureFlagsOptions {
    pub page: u64,

    #[default(MAX_LEGACY_FEATURES_PER_PAGE)]
    pub page_size: u64,

    #[serde(rename = "Type", skip_serializing_if = "Option::is_none")]
    pub feature_type: Option<LegacyFeatureFlagType>,

    #[serde(rename = "Code", skip_serializing_if = "Vec::is_empty")]
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, LegacyFeatureFlagId>")]
    #[default(LegacyFeatureFlagId::default_filter())]
    pub codes: Vec<LegacyFeatureFlagId>,
}

impl PaginateOptions for GetLegacyFeatureFlagsOptions {
    fn from_zero(page_size: u64) -> Self {
        Self {
            page: 0,
            page_size,
            ..Default::default()
        }
    }

    fn with_page(self, page: u64) -> Self {
        Self { page, ..self }
    }

    fn size(&self) -> u64 {
        self.page_size
    }
}

/// Request body for overriding a feature flag value.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PutFeatureFlagOverride {
    pub value: bool,
}

// ---------------------------------------------------------------------------
// Trait + blanket impl
// ---------------------------------------------------------------------------

#[allow(async_fn_in_trait)]
pub trait FeatureFlagsApi {
    async fn get_unleash_feature_flags(
        &self,
        request: GetUnleashFeaturesRequest,
    ) -> ApiServiceResult<GetUnleashFeaturesResponse>;

    async fn get_legacy_feature_flags(
        &self,
        options: GetLegacyFeatureFlagsOptions,
    ) -> ApiServiceResult<GetLegacyFeaturesResponse>;

    async fn put_feature_flag_override(
        &self,
        flag_name: &str,
        new_value: bool,
    ) -> ApiServiceResult<PutFeatureFlagOverrideResponse>;
}

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> FeatureFlagsApi for This {
    async fn get_unleash_feature_flags(
        &self,
        request: GetUnleashFeaturesRequest,
    ) -> ApiServiceResult<GetUnleashFeaturesResponse> {
        Ok(POST!("{UNLEASH_V2}/frontend")
            .body_json(request)?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_legacy_feature_flags(
        &self,
        options: GetLegacyFeatureFlagsOptions,
    ) -> ApiServiceResult<GetLegacyFeaturesResponse> {
        Ok(GET!("{CORE_V4}/features")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn put_feature_flag_override(
        &self,
        flag_name: &str,
        new_value: bool,
    ) -> ApiServiceResult<PutFeatureFlagOverrideResponse> {
        let request = PutFeatureFlagOverride { value: new_value };
        Ok(PUT!("{CORE_V4}/features/{flag_name}/value")
            .body_json(request)?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }
}
