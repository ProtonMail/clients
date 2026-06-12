use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, UnauthReq};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePostCodeReq {
    #[serde(flatten)]
    pub method: LtCoreCodeMethod,

    /// The platform for the verification link, optional.
    /// Can be "android" or other supported platforms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "Type")]
#[repr(C)]
pub enum LtCoreCodeMethod {
    Email(LtCoreCodeEmailMethod),
    Sms(LtCoreCodeSmsMethod),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreCodeEmailMethod {
    pub destination: LtCoreCodeDestinationEmail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreCodeSmsMethod {
    pub destination: LtCoreCodeDestinationSms,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreCodeDestinationEmail {
    /// The email address to send the verification code to.
    /// Required if the type is "email".
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCoreCodeDestinationSms {
    /// The phone number to send the verification code to.
    pub phone: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LtCorePostCodeRes {}

impl LtContract for LtCorePostCodeReq {
    type Response = LtSlimAPIJSON<LtCorePostCodeRes>;
    type Body<'a> = LtSlimAPIJSON<&'a Self>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(self)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/users/code"))
    }
}

impl UnauthReq for LtCorePostCodeReq {}
