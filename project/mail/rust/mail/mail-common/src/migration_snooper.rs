use anyhow::Error;
use async_trait::async_trait;
use futures::TryFutureExt;
use mail_core_api::services::proton::{ProtonCore, UserId};
use mail_core_common::{Context, migration_snooper::MigrationSnooper};
use mail_stash::AccountDb;
use mail_stash::macros::Model;
use mail_stash::orm::Model;
use mail_stash::stash::{Bond, StashError, Tether};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

pub struct MailMigrationSnooper {
    ctx: Arc<Context>,
}

impl MailMigrationSnooper {
    pub fn new(ctx: Arc<Context>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl MigrationSnooper for MailMigrationSnooper {
    async fn run(
        &self,
        user_id: &str,
        address_signature_enabled: Option<bool>,
        mobile_signature: Option<String>,
        mobile_signature_enabled: Option<bool>,
    ) -> Result<(), Error> {
        self.ctx
            .account_stash()
            .connection()
            .await?
            .tx(async |tx| {
                PostLoginMobileMigrationPayload {
                    user_id: user_id.into(),
                    address_signature_enabled,
                    mobile_signature,
                    mobile_signature_enabled,
                }
                .save(tx)
                .await
            })
            .await?;

        Ok(())
    }

    async fn run_post(&self, user_id: &str) -> Result<(), Error> {
        let sessions = self
            .ctx
            .get_account_sessions(user_id.into())
            .inspect_ok(|s| info!(n = s.len(), "loaded migrated sessions"))
            .inspect_err(|e| error!(?e, "failed to get account sessions"))
            .await?;

        // ET-6131: ensure migrated sessions have correct scopes.
        for s in sessions {
            if s.auth_scopes.is_empty() {
                match self.ctx.user_context_from_session(&s).await {
                    Ok(user_ctx) => {
                        // Nudge the session by making an API request.
                        // This will trigger auth refresh and populate the scopes.
                        warn!(?s.remote_id, "found stuck migration session");
                        let _ = user_ctx.session().get_users().await;
                    }

                    Err(error) => {
                        // We can't get the user context so the session is unusable.
                        // Logout locally so the user may log back in cleanly.
                        error!(?s.remote_id, ?error, "clearing stuck migration session");
                        let _ = self.ctx.force_logout_account_locally(s.account_id).await;
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Model)]
#[TableName("post_login_mobile_migration")]
#[Database(AccountDb)]
pub struct PostLoginMobileMigrationPayload {
    #[IdField]
    pub user_id: UserId,

    #[DbField]
    pub address_signature_enabled: Option<bool>,

    #[DbField]
    pub mobile_signature: Option<String>,

    #[DbField]
    pub mobile_signature_enabled: Option<bool>,
}

impl PostLoginMobileMigrationPayload {
    #[instrument(skip_all)]
    pub async fn load(id: &UserId, tether: &Tether<AccountDb>) -> Result<Option<Self>, StashError> {
        let exists: Option<i32> = tether
            .query_value_opt(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='post_login_mobile_migration'",
                Vec::new(),
            )
            .await?;

        if exists.is_none() {
            Ok(None)
        } else {
            <Self as Model>::load(id.clone(), tether).await
        }
    }

    #[instrument(skip_all)]
    pub async fn save(mut self, bond: &Bond<'_, AccountDb>) -> Result<(), StashError> {
        bond.execute(
            "CREATE TABLE IF NOT EXISTS post_login_mobile_migration (
                user_id STRING PRIMARY KEY,
                address_signature_enabled BOOL,
                mobile_signature TEXT,
                mobile_signature_enabled BOOL
             )",
            Vec::new(),
        )
        .await?;

        <Self as Model>::save(&mut self, bond).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_core_common::{CoreContextError, test_utils::test_context::TestContext};

    #[tokio::test]
    async fn test() {
        let ctx = TestContext::new().await;
        let mut tether = ctx.context().account_stash().connection().await.unwrap();

        assert_eq!(
            None,
            PostLoginMobileMigrationPayload::load(&"==abcd2".into(), &tether)
                .await
                .unwrap()
        );

        tether
            .tx::<_, _, CoreContextError>(async |tx| {
                for id in 0..5 {
                    PostLoginMobileMigrationPayload {
                        user_id: format!("==abcd{id}").into(),
                        address_signature_enabled: Some(true),
                        mobile_signature: Some("mobile signature".into()),
                        mobile_signature_enabled: Some(false),
                    }
                    .save(tx)
                    .await?;
                }

                Ok(())
            })
            .await
            .unwrap();

        assert_eq!(
            Some(PostLoginMobileMigrationPayload {
                user_id: "==abcd2".into(),
                address_signature_enabled: Some(true),
                mobile_signature: Some("mobile signature".into()),
                mobile_signature_enabled: Some(false),
            }),
            PostLoginMobileMigrationPayload::load(&"==abcd2".into(), &tether)
                .await
                .unwrap()
        );
    }
}
