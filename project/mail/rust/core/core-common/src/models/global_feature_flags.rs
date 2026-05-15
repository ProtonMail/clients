use mail_stash::{
    AccountDb,
    macros::Model,
    orm::Model,
    stash::{StashError, Tether, WriteTx},
};
use smart_default::SmartDefault;

use crate::datatypes::{FeatureFlagPayloadType, UnixTimestamp, Variant, VariantPayload};
use crate::models::ModelExtension;

#[derive(Debug, Clone, PartialEq, Model, SmartDefault)]
#[TableName("feature_flags")]
#[Database(AccountDb)]
pub struct FeatureFlag {
    #[IdField]
    pub name: String,

    #[DbField]
    pub enabled: bool,

    #[DbField]
    pub modify_time: UnixTimestamp,

    #[DbField]
    pub variant_name: Option<String>,

    #[DbField]
    pub variant_enabled: Option<bool>,

    #[DbField]
    pub variant_payload_type: Option<FeatureFlagPayloadType>,

    #[DbField]
    pub variant_payload_value: Option<String>,
}

impl FeatureFlag {
    pub async fn by_name(
        name: impl Into<String>,
        tether: &Tether<AccountDb>,
    ) -> Result<Option<Self>, StashError> {
        Self::find_by_id(name.into(), tether).await
    }

    pub async fn save_all(new: Vec<Self>, tx: &WriteTx<'_, AccountDb>) -> Result<(), StashError> {
        for mut flag in new {
            Self::save(&mut flag, tx).await?;
        }

        Ok(())
    }

    #[must_use]
    pub fn variant(&self) -> Option<Variant> {
        let name = self.variant_name.clone()?;
        let enabled = self.variant_enabled?;
        let payload = match (self.variant_payload_type, &self.variant_payload_value) {
            (Some(ty), Some(value)) => Some(VariantPayload {
                ty,
                value: value.clone(),
            }),
            _ => None,
        };
        Some(Variant {
            name,
            enabled,
            payload,
        })
    }
}
