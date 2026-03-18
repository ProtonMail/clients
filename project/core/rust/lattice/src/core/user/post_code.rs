use std::borrow::Cow;

use crate::{LatticeError, LtContract, LtSlimAPIJSON, Method, UnauthReq};

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "PascalCase")
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtCorePostCodeReq {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub method: LtCoreCodeMethod,

    /// The platform for the verification link, optional.
    /// Can be "android" or other supported platforms.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub platform: Option<String>,
}

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "PascalCase", tag = "Type")
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LtCoreCodeMethod {
    #[cfg_attr(feature = "serde", serde(rename = "email"))]
    Email {
        destination: LtCoreCodeDestinationEmail,
    },
    #[cfg_attr(feature = "serde", serde(rename = "sms"))]
    Sms {
        destination: LtCoreCodeDestinationSms,
    },
}

impl LtCoreCodeMethod {
    pub fn email(email: impl Into<String>) -> Self {
        Self::Email {
            destination: LtCoreCodeDestinationEmail {
                address: email.into(),
            },
        }
    }

    pub fn sms(phone: impl Into<String>) -> Self {
        Self::Sms {
            destination: LtCoreCodeDestinationSms {
                phone: phone.into(),
            },
        }
    }
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "PascalCase")
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtCoreCodeDestinationEmail {
    /// The email address to send the verification code to.
    /// Required if the type is "email".
    pub address: String,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "PascalCase")
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtCoreCodeDestinationSms {
    /// The phone number to send the verification code to.
    pub phone: String,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePostCodeRes {}

impl LtContract for LtCorePostCodeReq {
    type Response = LtSlimAPIJSON<LtCorePostCodeRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users/code"))
    }
}

impl UnauthReq for LtCorePostCodeReq {}
