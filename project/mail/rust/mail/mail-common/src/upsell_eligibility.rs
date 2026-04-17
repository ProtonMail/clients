use crate::{MailContextResult, MailUserContext};
use anyhow::Context;
use mail_core_common::datatypes::{UpsellEligibility, UpsellType};
use mail_core_common::models::{PaidSubscription, Role, User, UserSettings};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::{Stash, StashError, WatcherHandle};
use sqlite_watcher::watcher::TableObserver;
use std::collections::BTreeSet;
use std::sync::Weak;

// Two flags are needed because the API cannot distinguish "FF disabled" from "FF does not exist".
// Parent is conditioned by Unleash rules (e.g. user in Nordics). Child splits eligible users
// into control vs test group for telemetry:
//
//  Parent | Child | Result
//  -------+-------+-------------------------------
//  false  |   -   | Normal Plus upsell (baseline)
//  true   | false | Normal Plus upsell (control)
//  true   | true  | Unlimited upsell   (test)
pub const FF_UPSELL_UNLIMITED_PARENT: &str = "MailiosUnlimitedPlanPlacementEligibile";
pub const FF_UPSELL_UNLIMITED_CHILD: &str = "MailiosUnlimitedPlanPlacementTestGroup";

/// Note: This service is currently used only on iOS.
pub struct UpsellEligibilityService {
    ctx: Weak<MailUserContext>,
}

impl UpsellEligibilityService {
    pub fn new(ctx: Weak<MailUserContext>) -> Self {
        Self { ctx }
    }

    pub async fn watch_upsell_eligibility(&self) -> MailContextResult<WatcherHandle> {
        let ctx = self.ctx.upgrade().context("Could not find the context")?;
        Ok(UpsellEligibilityWatcher::watch(ctx.user_stash()).await?)
    }

    pub async fn upsell_eligibility(&self) -> MailContextResult<UpsellEligibility> {
        let ctx = self.ctx.upgrade().context("Could not find the context")?;
        let user = ctx.user().await?;

        if user.subscribed != PaidSubscription::empty() || user.role == Role::Member {
            Ok(UpsellEligibility::NotEligible)
        } else {
            let upsell_type = self.upsell_type().await?;
            Ok(UpsellEligibility::Eligible(upsell_type))
        }
    }

    async fn upsell_type(&self) -> MailContextResult<UpsellType> {
        let ctx = self.ctx.upgrade().context("Could not find the context")?;
        let feature_flags = ctx.user_context().feature_flags();

        if feature_flags
            .get(FF_UPSELL_UNLIMITED_PARENT)
            .await?
            .unwrap_or_default()
            && feature_flags
                .get(FF_UPSELL_UNLIMITED_CHILD)
                .await?
                .unwrap_or_default()
        {
            Ok(UpsellType::Unlimited)
        } else {
            Ok(UpsellType::MailPlus)
        }
    }
}

pub struct UpsellEligibilityWatcher;

impl UpsellEligibilityWatcher {
    pub async fn watch(mail_stash: &Stash<UserDb>) -> Result<WatcherHandle, StashError> {
        mail_stash
            .subscribe_to(|sender| Box::new(UpsellEligibilityTableWatcher { sender }))
            .await
    }
}

struct UpsellEligibilityTableWatcher {
    sender: flume::Sender<()>,
}

impl TableObserver for UpsellEligibilityTableWatcher {
    fn tables(&self) -> Vec<String> {
        vec![
            User::table_name().to_string(),
            UserSettings::table_name().to_string(),
        ]
    }

    fn on_tables_changed(&self, _changed_tables: &BTreeSet<String>) {
        self.sender
            .send(())
            .inspect_err(|e| {
                tracing::error!(
                    "Failed to send notification for UpsellEligibilityWatcher: {:?}",
                    e
                )
            })
            .ok();
    }
}
