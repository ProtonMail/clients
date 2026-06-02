use crate::mail_scroller::CATEGORY_VIEW_FEATURE_FLAG;
use mail_core_common::datatypes::{UnixTimestamp, UserFeatureFlagSource};
use mail_core_common::models::UserFeatureFlag;
use mail_stash::orm::Model;
use mail_stash::stash::{StashError, WriteTx};

pub async fn enable_category_view_ff(tx: &WriteTx<'_>) -> Result<(), StashError> {
    UserFeatureFlag::delete_batch_from_source(
        vec![CATEGORY_VIEW_FEATURE_FLAG.to_string()],
        UserFeatureFlagSource::Unleash,
        tx,
    )
    .await?;
    UserFeatureFlag::unleash(CATEGORY_VIEW_FEATURE_FLAG, UnixTimestamp::new(0))
        .save(tx)
        .await
}
