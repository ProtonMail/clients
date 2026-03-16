use anyhow::Error as AnyError;
use derive_more::Display;
use mail_muon::client::middleware::AuthErr;
use mail_muon::common::ParseEndpointErr;
use mail_muon::error::ErrorKind as MuonErrorKind;
use mail_muon::{Status, StatusErr};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_qs::Error as QueryStringError;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::string::FromUtf8Error;
use thiserror::Error;

/// A result containing an error that defaults to [`ApiServiceError`].
pub type ApiServiceResult<T, E = ApiServiceError> = Result<T, E>;

/// Additional information about an API service error.
#[derive(Clone, Debug, Display, Default, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(serde::Serialize))]
#[display("{code}: {error:?} ({details:?})")]
#[serde(rename_all = "PascalCase")]
pub struct ApiErrorInfo {
    /// Internal API code.
    pub code: u32,

    /// Optional error message that may be present.
    pub error: Option<String>,

    /// Optional JSON type with error details.
    pub details: Option<JsonValue>,
}

impl ApiErrorInfo {
    /// Parse the error from json data.
    pub fn from_json(json: impl AsRef<str>) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json.as_ref())
    }
}

/// The possible errors that can occur when using an external API.
#[derive(Debug, Error)]
pub enum ApiServiceError {
    //  NETWORK ERRORS
    //==========================================================================
    #[error("Network connection error: {0}")]
    ConnectionError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Redirect error for {0}: {1}")]
    Redirect(String, String),

    #[error("Timeout: {0}")]
    Timeout(String),

    //  PROTOCOL ERRORS
    //==========================================================================
    #[error("Bad request: {0}. {1:?}")]
    BadRequest(String, Option<ApiErrorInfo>),

    #[error("Unauthorized: {0}. {1:?}")]
    Unauthorized(String, Option<ApiErrorInfo>),

    #[error("Forbidden: {0}. {1:?}")]
    Forbidden(String, Option<ApiErrorInfo>),

    #[error("Not found: {0}. {1:?}")]
    NotFound(String, Option<ApiErrorInfo>),

    #[error("Unprocessable entity: {0}. {1:?}")]
    UnprocessableEntity(String, Option<ApiErrorInfo>),

    #[error("Too many requests: {0}. {1:?}")]
    TooManyRequests(String, Option<ApiErrorInfo>),

    #[error("Internal server error: {0}. {1:?}")]
    InternalServerError(String, Option<ApiErrorInfo>),

    #[error("Not Implemented: {0}. {1:?}")]
    NotImplemented(String, Option<ApiErrorInfo>),

    #[error("Bad gateway: {0}. {1:?}")]
    BadGateway(String, Option<ApiErrorInfo>),

    #[error("Service Unavailable: {0}. {1:?}")]
    ServiceUnavailable(String, Option<ApiErrorInfo>),

    #[error("HTTP error {0}: {1}. {2:?}")]
    OtherHttpError(Status, String, Option<ApiErrorInfo>),

    //  DATA ERRORS
    //==========================================================================
    #[error("Endpoint parsing error: {0}")]
    ParseEndpoint(#[from] ParseEndpointErr),

    #[error("Query encoding error: {0}")]
    QueryStringError(#[from] QueryStringError),

    #[error("Request composition error: {0}")]
    RequestError(String),

    #[error("Response parsing error: {0}")]
    ResponseError(String),

    #[error("UTF8 decoding error: {0}")]
    Utf8DecodingError(#[from] FromUtf8Error),

    //  LOGIC ERRORS
    //==========================================================================
    #[error("Authentication Store error: {0}")]
    AuthStore(#[from] AnyError),

    #[error("Unknown error: {0}")]
    UnknownError(String),
}

impl ApiServiceError {
    #[must_use]
    pub fn is_network_failure(&self) -> bool {
        matches!(
            self,
            Self::Redirect(_, _)
                | Self::Timeout(_)
                | Self::NetworkError(_)
                | Self::ConnectionError(_)
        )
    }

    #[must_use]
    pub fn is_auth_failure(&self) -> bool {
        matches!(
            self,
            ApiServiceError::Unauthorized(..) | ApiServiceError::Forbidden(..)
        )
    }

    #[must_use]
    pub fn is_server_failure(&self) -> bool {
        match self {
            Self::TooManyRequests(_, _)
            | Self::BadGateway(_, _)
            | Self::NotImplemented(_, _)
            | Self::ServiceUnavailable(_, _)
            | Self::InternalServerError(_, _) => true,
            Self::OtherHttpError(code, _, _) => code.as_u16() >= 500,
            _ => false,
        }
    }

    #[must_use]
    pub fn to_proton_error(&self) -> Option<ApiErrorInfo> {
        match self {
            Self::BadRequest(_, Some(e))
            | Self::Unauthorized(_, Some(e))
            | Self::Forbidden(_, Some(e))
            | Self::NotFound(_, Some(e))
            | Self::UnprocessableEntity(_, Some(e))
            | Self::TooManyRequests(_, Some(e))
            | Self::InternalServerError(_, Some(e))
            | Self::NotImplemented(_, Some(e))
            | Self::BadGateway(_, Some(e))
            | Self::ServiceUnavailable(_, Some(e))
            | Self::OtherHttpError(_, _, Some(e)) => Some(e.to_owned()),
            _ => None,
        }
    }
}

/// Marker trait for service errors.
pub trait ServiceError: Debug + Display + Send + Sync {}

#[allow(clippy::redundant_closure_for_method_calls)]
impl From<mail_muon::Error> for ApiServiceError {
    fn from(e: mail_muon::Error) -> Self {
        if e.source()
            .is_some_and(|s| s.is::<mail_muon::common::Timeout>())
        {
            return Self::Timeout(e.to_string());
        }

        if let Some(e) = e.source().and_then(|s| s.downcast_ref::<StatusErr>()) {
            return Self::from(e.to_owned());
        }

        if let Some(e) = e.source().and_then(|s| s.downcast_ref::<AuthErr>()) {
            return Self::Unauthorized(e.to_string(), None);
        }

        match e.kind() {
            MuonErrorKind::Tls
            | MuonErrorKind::Resolve
            | MuonErrorKind::Dial
            | MuonErrorKind::Connect => Self::ConnectionError(e.to_string()),
            MuonErrorKind::Send => Self::NetworkError(e.to_string()),
            MuonErrorKind::Req => Self::RequestError(e.to_string()),
            MuonErrorKind::Res => Self::ResponseError(e.to_string()),
            _ => Self::UnknownError(e.to_string()),
        }
    }
}

impl From<StatusErr> for ApiServiceError {
    fn from(value: StatusErr) -> Self {
        Self::from(&value)
    }
}

impl From<&StatusErr> for ApiServiceError {
    fn from(&StatusErr(code, ref res): &StatusErr) -> Self {
        macro_rules! err {
            ($body:expr) => {
                ApiErrorInfo::from_json($body).ok()
            };
        }

        let body = match String::from_utf8(res.body().to_owned()) {
            Ok(b) => b,
            Err(e) => return Self::Utf8DecodingError(e),
        };

        match (code, code.to_string()) {
            (code, e) if code.is_redirection() => Self::Redirect(e, body),
            (Status::BAD_REQUEST, e) => Self::BadRequest(e, err!(body)),
            (Status::UNAUTHORIZED, e) => Self::Unauthorized(e, err!(body)),
            (Status::FORBIDDEN, e) => Self::Forbidden(e, err!(body)),
            (Status::NOT_FOUND, e) => Self::NotFound(e, err!(body)),
            (Status::UNPROCESSABLE_ENTITY, e) => Self::UnprocessableEntity(e, err!(body)),
            (Status::TOO_MANY_REQUESTS, e) => Self::TooManyRequests(e, err!(body)),
            (Status::INTERNAL_SERVER_ERROR, e) => Self::InternalServerError(e, err!(body)),
            (Status::NOT_IMPLEMENTED, e) => Self::NotImplemented(e, err!(body)),
            (Status::BAD_GATEWAY, e) => Self::BadGateway(e, err!(body)),
            (Status::SERVICE_UNAVAILABLE, e) => Self::ServiceUnavailable(e, err!(body)),
            (code, e) => Self::OtherHttpError(code, e, err!(body)),
        }
    }
}
