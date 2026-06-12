use derive_more::Display;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
#[display(
    "HumanVerificationToken: {human_verification_token}, HumanVerificationMethods: {human_verification_methods:?}, Direct: {direct}, Description: {description}, Title: {title}, WebUrl: {web_url}, ExpiresAt: {expires_at}"
)]
pub struct HumanVerificationErrorDetails {
    human_verification_token: String,
    /// Backend-defined verification methods (e.g. `captcha`, `email`, `sms`, `ownership-email`,
    /// `ownership-sms`, `invite`, `coupon`). Kept as free-form strings -- the set is
    /// open-ended and the platform clients (Apple, Android, mail) all treat it as such (see
    /// e.g. `mail/rust/api/mail-api-session/src/challenge.rs::HumanVerificationChallenge`).
    /// Modelling it as a closed enum previously caused the entire HumanVerification error
    /// payload to fail deserialization and fall through to `LtApiResponseError::Other` when
    /// the backend returned any method other than `captcha`.
    human_verification_methods: Vec<String>,
    #[serde(with = "crate::helpers::bool_int")]
    direct: bool,
    description: String,
    title: String,
    web_url: String,
    expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[repr(C)]
#[allow(dead_code)] // wire/API value (e.g. from serde); not constructed in this crate
pub enum HumanVerificationMethod {
    #[display("captcha")]
    Captcha,
}
