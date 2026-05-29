use crate::{UniffiEnum, UniffiRecord};
use mail_core_common::datatypes::{
    FeatureFlagPayloadType as RealFeatureFlagPayloadType, Variant as RealVariant,
    VariantPayload as RealVariantPayload,
};

/// An Unleash feature flag variant.
///
/// Returned by `get_feature_flag_variant` to expose the active variant for a
/// feature flag. `name` is the variant identifier (e.g. "Unlimited_Nordics"),
/// `enabled` indicates whether the variant is active for the current user, and
/// `payload` carries optional metadata attached to the variant on the Unleash
/// dashboard.
#[derive(Clone, Debug, Eq, PartialEq, UniffiRecord)]
pub struct FeatureFlagVariant {
    pub name: String,
    pub enabled: bool,
    pub payload: Option<FeatureFlagVariantPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq, UniffiRecord)]
pub struct FeatureFlagVariantPayload {
    pub ty: FeatureFlagVariantPayloadType,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, UniffiEnum)]
pub enum FeatureFlagVariantPayloadType {
    Json,
    Csv,
    String,
    Number,
}

impl From<RealFeatureFlagPayloadType> for FeatureFlagVariantPayloadType {
    fn from(value: RealFeatureFlagPayloadType) -> Self {
        match value {
            RealFeatureFlagPayloadType::Json => Self::Json,
            RealFeatureFlagPayloadType::Csv => Self::Csv,
            RealFeatureFlagPayloadType::String => Self::String,
            RealFeatureFlagPayloadType::Number => Self::Number,
        }
    }
}

impl From<RealVariantPayload> for FeatureFlagVariantPayload {
    fn from(value: RealVariantPayload) -> Self {
        Self {
            ty: value.ty.into(),
            value: value.value,
        }
    }
}

impl From<RealVariant> for FeatureFlagVariant {
    fn from(value: RealVariant) -> Self {
        Self {
            name: value.name,
            enabled: value.enabled,
            payload: value.payload.map(Into::into),
        }
    }
}
