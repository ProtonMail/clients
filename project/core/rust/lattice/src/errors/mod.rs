mod enforced_code;
pub use enforced_code::EnforcedCode;
pub mod details;

use derive_more::{Display, Error};

use crate::details::{
    AccessTokenWithInsufficientScopeErrorDetails, HumanVerificationErrorDetails,
    LoginFailedErrorDetails,
};

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
#[derive(Debug, Clone, PartialEq, Eq, Display, Error)]
#[display("Error[{code}]: {error}, Specifics: {details}, Metadata: {metadata:?}")]
pub struct LtApiResponseErrorInfo<Code, Details> {
    pub code: Code,

    pub details: Details,

    pub error: String,

    #[cfg_attr(feature = "serde", serde(flatten))]
    pub metadata: LtApiErrorMetadata,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, PartialEq, Eq, Display, Error)]
#[display("File: {file:?}:{line:?} {exception:?} {message:?}")]
pub struct LtApiErrorMetadata {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub exception: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub message: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub file: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub line: Option<u32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub trace: Option<Vec<LtApiErrorMetadataTrace>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub previous: Option<Box<LtApiErrorMetadata>>,
}

// LrApiErrorMetadataTrace
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, PartialEq, Eq, Display)]
#[display("Trace: {file:?}:{line:?} {function:?}")]
pub struct LtApiErrorMetadataTrace {
    pub file: String,
    pub line: u32,
    pub function: String,
}

/// Error code: device is not active (device association).
pub const AUTH_DEVICE_NOT_ACTIVE: u32 = 10301;
/// Error code: device token is invalid (device association).
pub const AUTH_DEVICE_TOKEN_INVALID: u32 = 10302;
/// Error code: device is rejected (device association).
pub const AUTH_DEVICE_REJECTED: u32 = 10303;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, PartialEq, Eq, Display)]
#[cfg_attr(feature = "serde", serde(untagged, rename_all = "PascalCase"))]
pub enum LtApiResponseError {
    #[display("AccessTokenWithInsufficientScope")]
    AccessTokenWithInsufficientScope(
        LtApiResponseErrorInfo<EnforcedCode<9106>, AccessTokenWithInsufficientScopeErrorDetails>,
    ),

    #[display("InvalidDeviceID")]
    InvalidDeviceID(LtApiResponseErrorInfo<EnforcedCode<2061>, NullErrorDetails>),

    #[display("InvalidRequestJsonBody")]
    InvalidRequestJsonBody(LtApiResponseErrorInfo<EnforcedCode<6001>, NullErrorDetails>),

    #[display("LoginFailed")]
    LoginFailed(LtApiResponseErrorInfo<EnforcedCode<8002>, LoginFailedErrorDetails>),

    #[display("InvalidPayload")]
    InvalidPayload(LtApiResponseErrorInfo<EnforcedCode<2001>, NullErrorDetails>),

    #[display("DeviceAlreadyAssociated")]
    DeviceAlreadyAssociated(LtApiResponseErrorInfo<EnforcedCode<9107>, NullErrorDetails>),

    #[display("DeviceNotActive")]
    DeviceNotActive(LtApiResponseErrorInfo<EnforcedCode<AUTH_DEVICE_NOT_ACTIVE>, NullErrorDetails>),

    #[display("DeviceTokenInvalid")]
    DeviceTokenInvalid(
        LtApiResponseErrorInfo<EnforcedCode<AUTH_DEVICE_TOKEN_INVALID>, NullErrorDetails>,
    ),

    #[display("HumanVerification")]
    HumanVerification(LtApiResponseErrorInfo<EnforcedCode<9001>, HumanVerificationErrorDetails>),

    /// Email domain not found, please sign in with a password
    /// This is raised on SSO login when the email domain is not found.
    #[display("EmailDomainNotFound")]
    EmailDomainNotFound(LtApiResponseErrorInfo<EnforcedCode<8101>, NullErrorDetails>),

    /// Challenge corresponding to token not found
    /// This is raised on SSO login when the challenge corresponding to the token is not found.
    #[display("ChallengeNotFound")]
    ChallengeNotFound(LtApiResponseErrorInfo<EnforcedCode<2501>, NullErrorDetails>),

    /// This application is not supported, please contact your organization administrator.
    /// This is raised on SSO login when the application is not supported.
    #[display("ApplicationNotSupported")]
    ApplicationNotSupported(LtApiResponseErrorInfo<EnforcedCode<10402>, NullErrorDetails>),

    /// Your current plan does not support creating single sign-on domains
    /// This is raised on SSO login when the current plan does not support creating single sign-on domains.
    #[display("PlanNotSupported")]
    PlanNotSupported(LtApiResponseErrorInfo<EnforcedCode<2011>, NullErrorDetails>),

    #[display("Other")]
    Other(LtApiResponseErrorInfo<u32, serde_json::Value>),
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub struct NullErrorDetails {}
