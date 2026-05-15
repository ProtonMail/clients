use mail_stash::{
    UserDb,
    macros::Model,
    orm::Model,
    params,
    stash::{StashError, Tether, WriteTx},
    utils::{IterMapToSql, placeholders_n},
};

use crate::datatypes::{
    FeatureFlagPayloadType, FlagMutability, UnixTimestamp, UserFeatureFlagSource, Variant,
    VariantPayload,
};

#[derive(Debug, Clone, PartialEq, Model)]
#[TableName("user_feature_flags")]
#[Database(UserDb)]
pub struct UserFeatureFlag {
    #[IdField]
    pub name: String,

    #[DbField]
    pub enabled: bool,

    #[DbField]
    pub source: UserFeatureFlagSource,

    #[DbField]
    pub writable: bool,

    #[DbField]
    pub overridden_to: Option<bool>,

    #[DbField]
    pub overridden_at: Option<UnixTimestamp>,

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

impl UserFeatureFlag {
    #[must_use]
    pub fn unleash(name: impl Into<String>, modify_time: UnixTimestamp) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            source: UserFeatureFlagSource::Unleash,
            writable: false,
            overridden_to: None,
            overridden_at: None,
            modify_time,
            variant_name: None,
            variant_enabled: None,
            variant_payload_type: None,
            variant_payload_value: None,
        }
    }

    #[must_use]
    pub fn legacy(
        name: impl Into<String>,
        enabled: bool,
        mutability: FlagMutability,
        modify_time: UnixTimestamp,
    ) -> Self {
        Self {
            name: name.into(),
            enabled,
            source: UserFeatureFlagSource::Legacy,
            writable: mutability.to_writable(),
            overridden_to: None,
            overridden_at: None,
            modify_time,
            variant_name: None,
            variant_enabled: None,
            variant_payload_type: None,
            variant_payload_value: None,
        }
    }

    pub async fn by_name(
        name: impl Into<String>,
        tether: &Tether,
    ) -> Result<Option<Self>, StashError> {
        let name: String = name.into();
        Self::find_first(
            // If there are two flags with the same name, use Unleash one.
            "WHERE name = ? ORDER BY source ASC",
            params![name],
            tether,
        )
        .await
    }

    pub async fn save_all(new: Vec<Self>, tx: &WriteTx<'_>) -> Result<(), StashError> {
        for mut flag in new {
            Self::save(&mut flag, tx).await?;
        }

        Ok(())
    }

    pub async fn delete_batch_from_source(
        names: Vec<String>,
        source: UserFeatureFlagSource,
        tx: &WriteTx<'_>,
    ) -> Result<(), StashError> {
        tx.execute(
            format!(
                "DELETE FROM {} WHERE name IN ({}) AND source = ?",
                Self::table_name(),
                placeholders_n(names.len())
            ),
            names.bridge_sql_iter().chain(params![source]).collect(),
        )
        .await?;
        Ok(())
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.overridden_to.unwrap_or(self.enabled)
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
