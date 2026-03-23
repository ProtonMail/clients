use contacts_api::ContactSendingPreferences as ApiContactSendingPreferences;
use derive_more::TryFrom;
use mail_stash::exports::{
    FromSql, FromSqlError, FromSqlResult, SqliteError, ToSql, ToSqlOutput, Value, ValueRef,
};
use mail_stash::utils::sql_using_serde;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, TryFrom)]
#[try_from(repr)]
#[repr(u8)]
pub enum ContactSendingPreferences {
    Custom = 0,
    Default = 1,
}

impl From<ApiContactSendingPreferences> for ContactSendingPreferences {
    fn from(value: ApiContactSendingPreferences) -> Self {
        match value {
            ApiContactSendingPreferences::Custom => Self::Custom,
            ApiContactSendingPreferences::Default => Self::Default,
        }
    }
}

impl FromSql for ContactSendingPreferences {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let val = u8::column_result(value)?;
        Self::try_from(val).map_err(|_| FromSqlError::OutOfRange(i64::from(val)))
    }
}

impl ToSql for ContactSendingPreferences {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, SqliteError> {
        Ok(ToSqlOutput::Owned(Value::Integer(*self as i64)))
    }
}

/// Wrapper type around `Vec<String>` to implement [`FromSql`] and [`ToSql`].
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContactTypes(Vec<String>);

impl ContactTypes {
    /// Create a new [`ContactTypes`] instance from a list of [`String`]s.
    ///
    #[must_use]
    pub fn new(types: Vec<String>) -> Self {
        Self(types)
    }

    /// Convert the [`ContactTypes`] into the inner [`Vec`].
    #[must_use]
    pub fn into_inner(self) -> Vec<String> {
        self.0
    }
}

impl std::ops::Deref for ContactTypes {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

sql_using_serde!(ContactTypes);
