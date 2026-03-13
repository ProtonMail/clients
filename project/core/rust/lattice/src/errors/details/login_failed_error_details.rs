use derive_more::Display;

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
pub struct LoginFailedErrorDetails {
    pub login_failed_reason: LoginFailedReason,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
#[repr(C)]
pub enum LoginFailedReason {
    #[display("Wrong password")]
    WrongPassword,
    #[display("Address disabled")]
    AddressDisabled,
    #[display("Bad domain")]
    BadDomain,
    #[display("Wrong domain")]
    WrongDomain,
    #[display("Address not exist")]
    AddressNotExist,
    #[display("Address is a group")]
    AddressIsGroup,
    #[display("Username not exist")]
    UsernameNotExist,
    #[display("Wrong 2FA code")]
    TotpWrong,
    #[display("2FA code already used")]
    TotpReuse,
    #[display("Wrong recovery phrase")]
    RecoveryPhrase,
    #[display("Other")]
    Other,
}
