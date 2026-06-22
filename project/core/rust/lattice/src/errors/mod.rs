use serde::{Deserialize, Serialize};
mod enforced_code;
pub use enforced_code::EnforcedCode;
pub mod details;

use derive_more::{Display, Error};

use crate::details::{
    AccessTokenWithInsufficientScopeErrorDetails, HumanVerificationErrorDetails,
    LoginFailedErrorDetails,
};

#[derive(Debug, Clone, PartialEq, Eq, Display, Error, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
#[display("Error[{code}]: {error}, Specifics: {details}, Metadata: {metadata:?}")]
pub struct LtApiResponseErrorInfo<Code, Details> {
    pub code: Code,

    pub details: Details,

    pub error: String,

    #[serde(flatten)]
    pub metadata: LtApiErrorMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Error, Deserialize, Serialize)]
#[display("File: {file:?}:{line:?} {exception:?} {message:?}")]
pub struct LtApiErrorMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<LtApiErrorMetadataTrace>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<Box<LtApiErrorMetadata>>,
}

// LtApiErrorMetadataTrace
#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize, Serialize)]
#[display("Trace: {file:?}:{line:?} {function:?}")]
pub struct LtApiErrorMetadataTrace {
    pub file: String,
    pub line: u32,
    pub function: String,
}

// —— Proton / Core `Code` constants (for clients that match on numeric `Code`) ——
pub const ERROR_APP_VERSION_BAD: u32 = 5003;
pub const ERROR_AUTH_SWITCH_TO_SSO: u32 = 8100;
pub const ERROR_AUTH_SWITCH_TO_SRP: u32 = 8101;
pub const ERROR_UNPRIVATIZATION_NOT_EXISTS: u32 = 10401;
pub const ERROR_SSO_APPLICATION_INVALID: u32 = 10402;
pub const ERROR_SSO_CHALLENGE_NOT_FOUND: u32 = 2501;
pub const ERROR_SCOPE_MISSING_UNEXPECTED: u32 = 9100;
pub const ERROR_SCOPE_REAUTH_LOCKED: u32 = 9101;
/// Auth device: not found.
pub const AUTH_DEVICE_NOT_FOUND: u32 = 10300;
/// Error code: device is not active (device association).
pub const AUTH_DEVICE_NOT_ACTIVE: u32 = 10301;
/// Error code: device token is invalid (device association).
pub const AUTH_DEVICE_TOKEN_INVALID: u32 = 10302;
/// Error code: device is rejected (device association).
pub const AUTH_DEVICE_REJECTED: u32 = 10303;

#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize, Serialize)]
#[serde(untagged, rename_all = "PascalCase")]
pub enum LtApiResponseError {
    #[display("AccessTokenWithInsufficientScope")]
    AccessTokenWithInsufficientScope(
        LtApiResponseErrorInfo<EnforcedCode<9106>, AccessTokenWithInsufficientScopeErrorDetails>,
    ),

    #[display("InvalidID")]
    InvalidID(LtApiResponseErrorInfo<EnforcedCode<2061>, NullErrorDetails>),

    #[display("InvalidRequestJsonBody")]
    InvalidRequestJsonBody(LtApiResponseErrorInfo<EnforcedCode<6001>, NullErrorDetails>),

    #[display("LoginFailed")]
    LoginFailed(LtApiResponseErrorInfo<EnforcedCode<8002>, LoginFailedErrorDetails>),

    #[display("InvalidPayload")]
    InvalidPayload(LtApiResponseErrorInfo<EnforcedCode<2001>, NullErrorDetails>),

    #[display("DeviceAlreadyAssociated")]
    DeviceAlreadyAssociated(LtApiResponseErrorInfo<EnforcedCode<9107>, NullErrorDetails>),

    #[display("AppVersionBad")]
    AppVersionBad(LtApiResponseErrorInfo<EnforcedCode<ERROR_APP_VERSION_BAD>, NullErrorDetails>),

    /// `AUTH_SWITCH_TO_SSO` when the account requires SSO.
    #[display("AuthSwitchToSso")]
    AuthSwitchToSso(
        LtApiResponseErrorInfo<EnforcedCode<ERROR_AUTH_SWITCH_TO_SSO>, NullErrorDetails>,
    ),

    /// `AUTH_SWITCH_TO_SRP` when the account requires password / SRP.
    #[display("AuthSwitchToSrp")]
    AuthSwitchToSrp(
        LtApiResponseErrorInfo<EnforcedCode<ERROR_AUTH_SWITCH_TO_SRP>, NullErrorDetails>,
    ),

    #[display("UnprivatizationNotExists")]
    UnprivatizationNotExists(
        LtApiResponseErrorInfo<EnforcedCode<ERROR_UNPRIVATIZATION_NOT_EXISTS>, NullErrorDetails>,
    ),

    #[display("ScopeMissingUnexpected")]
    ScopeMissingUnexpected(
        LtApiResponseErrorInfo<EnforcedCode<ERROR_SCOPE_MISSING_UNEXPECTED>, NullErrorDetails>,
    ),

    #[display("ScopeReauthLocked")]
    ScopeReauthLocked(
        LtApiResponseErrorInfo<
            EnforcedCode<ERROR_SCOPE_REAUTH_LOCKED>,
            AccessTokenWithInsufficientScopeErrorDetails,
        >,
    ),

    #[display("DeviceNotFound")]
    DeviceNotFound(LtApiResponseErrorInfo<EnforcedCode<AUTH_DEVICE_NOT_FOUND>, NullErrorDetails>),

    #[display("DeviceNotActive")]
    DeviceNotActive(LtApiResponseErrorInfo<EnforcedCode<AUTH_DEVICE_NOT_ACTIVE>, NullErrorDetails>),

    #[display("DeviceTokenInvalid")]
    DeviceTokenInvalid(
        LtApiResponseErrorInfo<EnforcedCode<AUTH_DEVICE_TOKEN_INVALID>, NullErrorDetails>,
    ),

    #[display("DeviceRejected")]
    DeviceRejected(LtApiResponseErrorInfo<EnforcedCode<AUTH_DEVICE_REJECTED>, NullErrorDetails>),

    #[display("HumanVerification")]
    HumanVerification(LtApiResponseErrorInfo<EnforcedCode<9001>, HumanVerificationErrorDetails>),

    #[display("ChallengeNotFound")]
    ChallengeNotFound(
        LtApiResponseErrorInfo<EnforcedCode<ERROR_SSO_CHALLENGE_NOT_FOUND>, NullErrorDetails>,
    ),

    /// `SSO_APPLICATION_INVALID` — disallowed app for org SSO.
    #[display("SsoApplicationInvalid")]
    SsoApplicationInvalid(
        LtApiResponseErrorInfo<EnforcedCode<ERROR_SSO_APPLICATION_INVALID>, NullErrorDetails>,
    ),

    /// `NOT_ALLOWED` (2011) — e.g. plan does not support SSO domain setup.
    #[display("PlanNotSupported")]
    PlanNotSupported(LtApiResponseErrorInfo<EnforcedCode<2011>, NullErrorDetails>),

    #[display("Other")]
    Other(LtApiResponseErrorInfo<u32, serde_json::Value>),
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize, Serialize)]
pub struct NullErrorDetails {}
