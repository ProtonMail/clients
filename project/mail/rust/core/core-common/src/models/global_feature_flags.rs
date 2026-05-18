use mail_stash::AccountDb;
use mail_stash::macros::Model;
use mail_stash::orm::Model;
use mail_stash::stash::{StashError, Tether, WriteTx};
use smart_default::SmartDefault;

use crate::datatypes::UnixTimestamp;
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
}
