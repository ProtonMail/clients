mod enforced_code;
pub use enforced_code::EnforcedCode;
pub mod details;

use derive_more::{Display, Error};

use crate::details::{AccessTokenWithInsufficientScopeErrorDetails, LoginFailedErrorDetails};

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

    #[display("Other")]
    Other(LtApiResponseErrorInfo<u32, serde_json::Value>),
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub struct NullErrorDetails {}
