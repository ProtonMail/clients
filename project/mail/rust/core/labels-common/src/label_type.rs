use derive_more::derive::TryFrom;
use mail_core_api::services::proton::{LabelId, LabelType as ApiLabelType};
use mail_stash::exports::{
    FromSql, FromSqlError, FromSqlResult, SqliteError, ToSql, ToSqlOutput, Value, ValueRef,
};
use mail_stash::utils::sql_using_serde;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::ops::Deref;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, TryFrom)]
#[try_from(repr)]
#[repr(u8)]
pub enum LabelType {
    Label = 1,
    ContactGroup = 2,
    Folder = 3,
    System = 4,
}

impl Display for LabelType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Label => write!(f, "Label"),
            Self::ContactGroup => write!(f, "Contact Group"),
            Self::Folder => write!(f, "Folder"),
            Self::System => write!(f, "System"),
        }
    }
}

impl From<ApiLabelType> for LabelType {
    fn from(value: ApiLabelType) -> Self {
        match value {
            ApiLabelType::Label => Self::Label,
            ApiLabelType::ContactGroup => Self::ContactGroup,
            ApiLabelType::Folder => Self::Folder,
            ApiLabelType::System => Self::System,
        }
    }
}

impl From<LabelType> for ApiLabelType {
    fn from(value: LabelType) -> Self {
        match value {
            LabelType::Label => Self::Label,
            LabelType::ContactGroup => Self::ContactGroup,
            LabelType::Folder => Self::Folder,
            LabelType::System => Self::System,
        }
    }
}

impl FromSql for LabelType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let val = u8::column_result(value)?;
        Self::try_from(val).map_err(|_| FromSqlError::OutOfRange(i64::from(val)))
    }
}

impl ToSql for LabelType {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, SqliteError> {
        Ok(ToSqlOutput::Owned(Value::Integer(*self as i64)))
    }
}

pub const ALL_LABEL_TYPES: [LabelType; 4] = [
    LabelType::Label,
    LabelType::ContactGroup,
    LabelType::Folder,
    LabelType::System,
];
pub const MAIL_LABEL_TYPES: [LabelType; 3] =
    [LabelType::Label, LabelType::Folder, LabelType::System];
pub const CONTACT_LABEL_TYPES: [LabelType; 1] = [LabelType::ContactGroup];

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct LabelColor(String);

impl LabelColor {
    #[must_use]
    pub fn purple() -> Self {
        Self("#8080FF".into())
    }
    #[must_use]
    pub fn black() -> Self {
        Self("#000000".into())
    }
}

impl Display for LabelColor {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for LabelColor {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for LabelColor {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl FromSql for LabelColor {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().map(|s| LabelColor(s.to_string()))
    }
}

impl ToSql for LabelColor {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, SqliteError> {
        Ok(ToSqlOutput::from(self.0.clone()))
    }
}

/// Wrapper type around `Vec<RemoteId>` to implement [`FromSql`] and [`ToSql`].
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Labels(Vec<LabelId>);

impl Labels {
    /// Create a new [`Labels`] instance from a list of [`LabelId`]s.
    #[must_use]
    pub fn new(ids: Vec<LabelId>) -> Self {
        Self(ids)
    }

    /// Convert the [`Labels`] into the inner [`Vec`].
    #[must_use]
    pub fn into_inner(self) -> Vec<LabelId> {
        self.0
    }
}

impl Deref for Labels {
    type Target = Vec<LabelId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

sql_using_serde!(Labels);
