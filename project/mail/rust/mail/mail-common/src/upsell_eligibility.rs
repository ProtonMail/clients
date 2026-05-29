use crate::{MailContextResult, MailUserContext};
use anyhow::Context;
use mail_core_common::datatypes::{FeatureFlagPayloadType, UpsellEligibility, UpsellType, Variant};
use mail_core_common::models::{PaidSubscription, Role, User, UserSettings};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::{Stash, StashError, WatcherHandle};
use serde::Deserialize;
use sqlite_watcher::watcher::TableObserver;
use std::collections::BTreeSet;
use std::sync::Weak;

// Single Unleash flag whose variants steer upsell experiments. Each variant's JSON payload
// carries `{"upsell": "MailPlus" | "Unlimited"}` to pick the modal. Telemetry separately
// records the variant name (e.g. "Unlimited_Nordics", "MailPlus_USA") so analysts can split
// cohorts without further Rust changes.
pub const FF_UPSELL_EXPERIMENT: &str = "MailiosUpsellExperiment";

#[derive(Deserialize)]
struct UpsellPayload {
    upsell: UpsellType,
}

fn parse_upsell_payload(variant: &Variant) -> Option<UpsellType> {
    if !variant.enabled {
        return None;
    }
    let payload = variant.payload.as_ref()?;
    if payload.ty != FeatureFlagPayloadType::Json {
        tracing::error!(
            variant = %variant.name,
            payload_type = ?payload.ty,
            "upsell variant payload is not JSON, falling back to MailPlus"
        );
        return None;
    }
    match serde_json::from_str::<UpsellPayload>(&payload.value) {
        Ok(p) => Some(p.upsell),
        Err(err) => {
            tracing::error!(
                variant = %variant.name,
                error = %err,
                "malformed upsell variant payload, falling back to MailPlus"
            );
            None
        }
    }
}

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
        let variant = ctx
            .user_context()
            .feature_flags()
            .get_feature_flag_variant(FF_UPSELL_EXPERIMENT)
            .await?;
        Ok(variant
            .as_ref()
            .and_then(parse_upsell_payload)
            .unwrap_or(UpsellType::MailPlus))
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
