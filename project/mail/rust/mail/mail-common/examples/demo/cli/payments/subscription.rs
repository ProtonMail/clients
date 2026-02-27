use anyhow::Result;
use mail_common::MailUserContext;
use mail_core_api::services::proton::ProtonPayments;
use std::sync::Arc;

/// Display the active subscription for the given user.
#[derive(Debug, Args)]
pub struct Cmd {}

impl Cmd {
    pub async fn run(self, ctx: Arc<MailUserContext>) -> Result<()> {
        let plan = ctx.session().get_payments_subscription().await?;

        println!("{plan:#?}");

        Ok(())
    }
}
