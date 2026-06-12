use derive_more::Display;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoginFailedErrorDetails {
    pub login_failed_reason: LoginFailedReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, Deserialize, Serialize)]
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
