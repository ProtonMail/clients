use derive_more::TryFrom;
use mail_core_api::services::proton::UnleashTogglePayloadType;
use mail_stash::exports::{
    FromSql, FromSqlError, FromSqlResult, SqliteError, ToSql, ToSqlOutput, Value, ValueRef,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, TryFrom)]
#[try_from(repr)]
#[repr(u8)]
pub enum FeatureFlagPayloadType {
    Json = 0,
    Csv = 1,
    String = 2,
    Number = 3,
}

impl FromSql for FeatureFlagPayloadType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let val = u8::column_result(value)?;
        Self::try_from(val).map_err(|_| FromSqlError::OutOfRange(i64::from(val)))
    }
}

impl ToSql for FeatureFlagPayloadType {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, SqliteError> {
        Ok(ToSqlOutput::Owned(Value::Integer(*self as i64)))
    }
}

impl From<UnleashTogglePayloadType> for FeatureFlagPayloadType {
    fn from(value: UnleashTogglePayloadType) -> Self {
        match value {
            UnleashTogglePayloadType::Json => Self::Json,
            UnleashTogglePayloadType::Csv => Self::Csv,
            UnleashTogglePayloadType::String => Self::String,
            UnleashTogglePayloadType::Number => Self::Number,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variant {
    pub name: String,
    pub enabled: bool,
    pub payload: Option<VariantPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantPayload {
    pub ty: FeatureFlagPayloadType,
    pub value: String,
}
