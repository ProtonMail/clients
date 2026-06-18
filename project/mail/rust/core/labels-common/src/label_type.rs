use derive_more::derive::TryFrom;
use mail_core_api::services::proton::{LabelId, LabelType as ApiLabelType};
use mail_stash::exports::{
    FromSql, FromSqlError, FromSqlResult, SqliteError, ToSql, ToSqlOutput, Value, ValueRef,
};
use mail_stash::utils::sql_using_serde;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::fmt::{self, Display, Formatter};
use std::ops::Deref;

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    TryFrom,
    Deserialize_repr,
    Serialize_repr
)]
#[try_from(repr)]
#[repr(u8)]
pub enum LabelType {
    Label = 1,
    Folder = 3,
    System = 4,
}

impl Display for LabelType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Label => write!(f, "Label"),
            Self::Folder => write!(f, "Folder"),
            Self::System => write!(f, "System"),
        }
    }
}

impl From<ApiLabelType> for LabelType {
    fn from(value: ApiLabelType) -> Self {
        match value {
            ApiLabelType::Label => Self::Label,
            ApiLabelType::ContactGroup => panic!("Should not be used"),
            ApiLabelType::Folder => Self::Folder,
            ApiLabelType::System => Self::System,
        }
    }
}

impl From<LabelType> for ApiLabelType {
    fn from(value: LabelType) -> Self {
        match value {
            LabelType::Label => Self::Label,
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

pub const ALL_LABEL_TYPES: [LabelType; 3] =
    [LabelType::Label, LabelType::Folder, LabelType::System];
pub const MAIL_LABEL_TYPES: [LabelType; 3] =
    [LabelType::Label, LabelType::Folder, LabelType::System];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum WellKnownLabelColor {
    Purple,
    Pink,
    Strawberry,
    Carrot,
    Sahara,
    Enzian,
    Plum,
    Cerise,
    Copper,
    Soil,
    Slateblue,
    Pacific,
    Reef,
    Fern,
    Olive,
    Cobalt,
    Ocean,
    Pine,
    Forest,
    Pickle,
}

impl WellKnownLabelColor {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Purple => "purple",
            Self::Pink => "pink",
            Self::Strawberry => "strawberry",
            Self::Carrot => "carrot",
            Self::Sahara => "sahara",
            Self::Enzian => "enzian",
            Self::Plum => "plum",
            Self::Cerise => "cerise",
            Self::Copper => "copper",
            Self::Soil => "soil",
            Self::Slateblue => "slateblue",
            Self::Pacific => "pacific",
            Self::Reef => "reef",
            Self::Fern => "fern",
            Self::Olive => "olive",
            Self::Cobalt => "cobalt",
            Self::Ocean => "ocean",
            Self::Pine => "pine",
            Self::Forest => "forest",
            Self::Pickle => "pickle",
        }
    }

    pub fn hex_code(&self) -> &'static str {
        match self {
            Self::Purple => "#8080FF",
            Self::Pink => "#DB60D6",
            Self::Strawberry => "#EC3E7C",
            Self::Carrot => "#F78400",
            Self::Sahara => "#936D58",
            Self::Enzian => "#5252CC",
            Self::Plum => "#A839A4",
            Self::Cerise => "#BA1E55",
            Self::Copper => "#C44800",
            Self::Soil => "#54473F",
            Self::Slateblue => "#415DF0",
            Self::Pacific => "#179FD9",
            Self::Reef => "#1DA583",
            Self::Fern => "#3CBB3A",
            Self::Olive => "#B4A40E",
            Self::Cobalt => "#273EB2",
            Self::Ocean => "#0A77A6",
            Self::Pine => "#0F735A",
            Self::Forest => "#258723",
            Self::Pickle => "#807304",
        }
    }

    pub fn from_hex_code(color_code: &str) -> Option<Self> {
        match color_code {
            "#8080FF" => Some(Self::Purple),
            "#DB60D6" => Some(Self::Pink),
            "#EC3E7C" => Some(Self::Strawberry),
            "#F78400" => Some(Self::Carrot),
            "#936D58" => Some(Self::Sahara),
            "#5252CC" => Some(Self::Enzian),
            "#A839A4" => Some(Self::Plum),
            "#BA1E55" => Some(Self::Cerise),
            "#C44800" => Some(Self::Copper),
            "#54473F" => Some(Self::Soil),
            "#415DF0" => Some(Self::Slateblue),
            "#179FD9" => Some(Self::Pacific),
            "#1DA583" => Some(Self::Reef),
            "#3CBB3A" => Some(Self::Fern),
            "#B4A40E" => Some(Self::Olive),
            "#273EB2" => Some(Self::Cobalt),
            "#0A77A6" => Some(Self::Ocean),
            "#0F735A" => Some(Self::Pine),
            "#258723" => Some(Self::Forest),
            "#807304" => Some(Self::Pickle),
            _ => None,
        }
    }
}

impl From<WellKnownLabelColor> for LabelColor {
    fn from(value: WellKnownLabelColor) -> Self {
        Self(value.hex_code().into())
    }
}

#[derive(
    Clone,
    Debug,
    Default,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize
)]
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

#[cfg(test)]
mod tests {
    #[test]
    fn color_codes() {
        fn color_code_round_trip(c: super::WellKnownLabelColor) {
            assert_eq!(
                c,
                super::WellKnownLabelColor::from_hex_code(c.hex_code()).unwrap()
            );
        }
        color_code_round_trip(super::WellKnownLabelColor::Purple);
        color_code_round_trip(super::WellKnownLabelColor::Pink);
        color_code_round_trip(super::WellKnownLabelColor::Strawberry);
        color_code_round_trip(super::WellKnownLabelColor::Carrot);
        color_code_round_trip(super::WellKnownLabelColor::Sahara);
        color_code_round_trip(super::WellKnownLabelColor::Enzian);
        color_code_round_trip(super::WellKnownLabelColor::Plum);
        color_code_round_trip(super::WellKnownLabelColor::Cerise);
        color_code_round_trip(super::WellKnownLabelColor::Copper);
        color_code_round_trip(super::WellKnownLabelColor::Soil);
        color_code_round_trip(super::WellKnownLabelColor::Slateblue);
        color_code_round_trip(super::WellKnownLabelColor::Pacific);
        color_code_round_trip(super::WellKnownLabelColor::Reef);
        color_code_round_trip(super::WellKnownLabelColor::Fern);
        color_code_round_trip(super::WellKnownLabelColor::Olive);
        color_code_round_trip(super::WellKnownLabelColor::Cobalt);
        color_code_round_trip(super::WellKnownLabelColor::Ocean);
        color_code_round_trip(super::WellKnownLabelColor::Pine);
        color_code_round_trip(super::WellKnownLabelColor::Forest);
        color_code_round_trip(super::WellKnownLabelColor::Pickle);
    }
}
