//! Application-level authentication mode types.
//!
//! These types are the application-level counterparts to the API-level
//! authentication mode enums, enabling them to be stored in a database.

use derive_more::derive::TryFrom;
use serde::{Deserialize, Serialize};

use crate::auth::PasswordMode as ApiPasswordMode;
use crate::store::{MbpMode, TfaMode};

/// A compat type for the [`ApiPasswordMode`] enum, enabling it to be used
/// within the database.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, TryFrom)]
#[try_from(repr)]
#[repr(u8)]
pub enum PasswordMode {
    #[default]
    One = 1,
    Two = 2,
}

impl PasswordMode {
    /// Returns true if any type of additional password is active.
    #[must_use]
    pub fn has_mbp(self) -> bool {
        !matches!(self, Self::One)
    }
}

impl From<MbpMode> for PasswordMode {
    fn from(value: MbpMode) -> Self {
        match value {
            MbpMode::One => Self::One,
            MbpMode::Two => Self::Two,
        }
    }
}

impl From<PasswordMode> for MbpMode {
    fn from(value: PasswordMode) -> Self {
        match value {
            PasswordMode::One => MbpMode::One,
            PasswordMode::Two => MbpMode::Two,
        }
    }
}

impl From<ApiPasswordMode> for PasswordMode {
    fn from(value: ApiPasswordMode) -> Self {
        match value {
            ApiPasswordMode::One => Self::One,
            ApiPasswordMode::Two => Self::Two,
        }
    }
}

impl From<PasswordMode> for ApiPasswordMode {
    fn from(value: PasswordMode) -> Self {
        match value {
            PasswordMode::One => ApiPasswordMode::One,
            PasswordMode::Two => ApiPasswordMode::Two,
        }
    }
}

#[cfg(feature = "sql")]
impl mail_stash::exports::ToSql for PasswordMode {
    fn to_sql(
        &self,
    ) -> Result<mail_stash::exports::ToSqlOutput<'_>, mail_stash::exports::SqliteError> {
        Ok((*self as u8).into())
    }
}

#[cfg(feature = "sql")]
impl mail_stash::exports::FromSql for PasswordMode {
    fn column_result(
        value: mail_stash::exports::ValueRef<'_>,
    ) -> mail_stash::exports::FromSqlResult<Self> {
        let mail_stash::exports::ValueRef::Integer(value) = value else {
            return Err(mail_stash::exports::FromSqlError::InvalidType);
        };

        let Ok(value) = u8::try_from(value) else {
            return Err(mail_stash::exports::FromSqlError::InvalidType);
        };

        let Ok(value) = Self::try_from(value) else {
            return Err(mail_stash::exports::FromSqlError::InvalidType);
        };

        Ok(value)
    }
}

/// TODO: Document this enum.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize, TryFrom)]
#[try_from(repr)]
#[repr(u8)]
pub enum TfaStatus {
    /// TODO: Document this variant.
    #[default]
    None = 0,

    /// TODO: Document this variant.
    Totp = 1,

    /// TODO: Document this variant.
    Fido2 = 2,

    /// TODO: Document this variant.
    TotpOrFido2 = 3,
}

impl TfaStatus {
    /// Returns true if any type of second factor auth method is active.
    #[must_use]
    pub fn has_tfa(self) -> bool {
        !matches!(self, Self::None)
    }

    /// Returns true if TOTP is enabled.
    #[must_use]
    pub fn has_totp(self) -> bool {
        matches!(self, Self::Totp | Self::TotpOrFido2)
    }

    /// Returns true if FIDO2 is enabled.
    #[must_use]
    pub fn has_fido(self) -> bool {
        matches!(self, Self::Fido2 | Self::TotpOrFido2)
    }
}

impl From<TfaMode> for TfaStatus {
    fn from(value: TfaMode) -> Self {
        match (value.totp, value.fido) {
            (true, true) => Self::TotpOrFido2,
            (true, false) => Self::Totp,
            (false, true) => Self::Fido2,
            (false, false) => Self::None,
        }
    }
}

#[cfg(feature = "sql")]
impl mail_stash::exports::FromSql for TfaStatus {
    fn column_result(
        value: mail_stash::exports::ValueRef<'_>,
    ) -> mail_stash::exports::FromSqlResult<Self> {
        let val = <u8 as mail_stash::exports::FromSql>::column_result(value)?;
        Self::try_from(val)
            .map_err(|_| mail_stash::exports::FromSqlError::OutOfRange(i64::from(val)))
    }
}

#[cfg(feature = "sql")]
impl mail_stash::exports::ToSql for TfaStatus {
    fn to_sql(
        &self,
    ) -> Result<mail_stash::exports::ToSqlOutput<'_>, mail_stash::exports::SqliteError> {
        Ok(mail_stash::exports::ToSqlOutput::Owned(
            mail_stash::exports::Value::Integer(*self as i64),
        ))
    }
}
