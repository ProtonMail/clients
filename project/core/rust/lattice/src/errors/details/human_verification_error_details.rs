use derive_more::Display;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
#[display(
    "HumanVerificationToken: {human_verification_token}, HumanVerificationMethods: {human_verification_methods:?}, Direct: {direct}, Description: {description}, Title: {title}, WebUrl: {web_url}, ExpiresAt: {expires_at}"
)]
pub struct HumanVerificationErrorDetails {
    human_verification_token: String,
    human_verification_methods: Vec<HumanVerificationMethod>,
    #[cfg_attr(feature = "serde", serde(with = "crate::helpers::bool_int"))]
    direct: bool,
    description: String,
    title: String,
    web_url: String,
    expires_at: u64,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
#[repr(C)]
#[allow(dead_code)] // wire/API value (e.g. from serde); not constructed in this crate
pub enum HumanVerificationMethod {
    #[display("captcha")]
    Captcha,
}
