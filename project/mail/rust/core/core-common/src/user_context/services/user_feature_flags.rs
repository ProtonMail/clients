//! Note: This service is for per-user feature flags.
//! If you are looking for global feature flags,
//! please see [`crate::services::FeatureFlagsService`].

use std::collections::BTreeMap;
use std::sync::Weak;

use anyhow::Context as _;
use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::{
    FeatureFlagsApi as _, GetLegacyFeatureFlagsOptions, GetLegacyFeaturesResponse,
    GetUnleashFeaturesResponse, LegacyFeatureFlag, LegacyFeatureFlagType,
    MAX_LEGACY_FEATURES_PER_PAGE, UnleashToggleVariant,
};
use mail_core_api::session::Session;
use mail_stash::orm::Model;
use mail_stash::stash::{Stash, Tether, WatcherHandle};
use mail_stash::watcher::TableWatcher;
use mail_stash::{UserDb, params};

use crate::datatypes::{UnixTimestamp, UserFeatureFlagSource, Variant};
use crate::models::{ModelExtension, UserFeatureFlag};
use crate::services::FeatureFlagsService;
use crate::utils::Paginatable;
use crate::{Context, CoreContextError, CoreContextResult, UserContext};

enum FlagPersistence {
    Persist,
    DontPersist,
}

#[derive(Clone)]
pub struct UserFeatureFlagsService {
    ctx: Weak<UserContext>,
}

impl UserFeatureFlagsService {
    #[must_use]
    pub fn new(ctx: Weak<UserContext>) -> Self {
        Self { ctx }
    }

    async fn fetch_from_cache(
        tether: &Tether,
        source: UserFeatureFlagSource,
    ) -> BTreeMap<String, UserFeatureFlag> {
        UserFeatureFlag::find("where source=?", params![source], tether)
            .await
            .inspect_err(|err| tracing::warn!("Failed to fetch feature flags: {}", err))
            .unwrap_or_default()
            .into_iter()
            .map(|flag| (flag.name.clone(), flag))
            .collect::<BTreeMap<String, UserFeatureFlag>>()
    }

    fn set_flags_from_unleash(
        flags: &mut BTreeMap<String, UserFeatureFlag>,
        response: GetUnleashFeaturesResponse,
        modify_time: UnixTimestamp,
    ) {
        tracing::debug!(
            "Fetched {} user featured flags from unleash API",
            response.toggles.len()
        );
        for toggle in response.toggles {
            let flag = flags
                .entry(toggle.name.clone())
                .or_insert_with(|| UserFeatureFlag {
                    id: None,
                    name: toggle.name,
                    enabled: false,
                    source: UserFeatureFlagSource::Unleash,
                    writable: false,
                    overridden_to: None,
                    overridden_at: None,
                    modify_time,
                    variant_name: None,
                    variant_enabled: None,
                    variant_payload_type: None,
                    variant_payload_value: None,
                });

            let UnleashToggleVariant {
                name,
                enabled,
                feature_enabled,
                payload,
            } = toggle.variant;
            let (payload_type, payload_value) = match payload {
                Some(p) => (Some(p.ty.into()), Some(p.value)),
                None => (None, None),
            };
            flag.enabled = feature_enabled;
            flag.source = UserFeatureFlagSource::Unleash;
            flag.writable = false;
            flag.modify_time = modify_time;
            flag.variant_name = Some(name);
            flag.variant_enabled = Some(enabled);
            flag.variant_payload_type = payload_type;
            flag.variant_payload_value = payload_value;
        }
    }

    fn set_flags_from_legacy(
        flags: &mut BTreeMap<String, (UserFeatureFlag, FlagPersistence)>,
        api_flags: Vec<LegacyFeatureFlag>,
        now: UnixTimestamp,
    ) {
        let boolean_features = api_flags
            .into_iter()
            .filter(|feature| {
                let Some(expiration_time) =
                    feature.metadata.expiration_time.map(UnixTimestamp::from)
                else {
                    return true;
                };
                expiration_time >= now
            })
            .filter_map(|feature| {
                let LegacyFeatureFlag { metadata, variant } = feature;
                // Currently we support only boolean feature flags.
                let value = variant.into_bool();
                value.map(|value| (metadata, value))
            });

        for (metadata, value) in boolean_features {
            let enabled = value.value;

            let (flag, persistence) = flags.entry(metadata.code.clone()).or_insert_with(|| {
                (
                    UserFeatureFlag {
                        id: None,
                        name: metadata.code,
                        enabled,
                        source: UserFeatureFlagSource::Legacy,
                        writable: metadata.writable,
                        overridden_to: None,
                        overridden_at: None,
                        modify_time: now,
                        variant_name: None,
                        variant_enabled: None,
                        variant_payload_type: None,
                        variant_payload_value: None,
                    },
                    FlagPersistence::Persist,
                )
            });

            *persistence = FlagPersistence::Persist;
            if let Some(overridden_at) = flag.overridden_at
                && let Some(remote_update_at) = metadata.update_time
            {
                // Both dates come from the same source - the backend.
                // We never update those fields with device clock.
                // Therefore it is safe to compare those two timestamps.
                let remote_updated_at = UnixTimestamp::from(remote_update_at);
                if overridden_at > remote_updated_at {
                    // This is stale data.
                    tracing::warn!("Stale data for feature flag {}", flag.name);
                    tracing::warn!("Overridden at: {}", overridden_at);
                    tracing::warn!("Remote update at: {}", remote_update_at);
                    tracing::warn!("Flag stays as: {}", flag.enabled);
                    continue;
                }
            }
            flag.enabled = enabled;
            flag.source = UserFeatureFlagSource::Legacy;
            flag.writable = metadata.writable;
            flag.modify_time = now;

            // Overridden at is set only AFTER remote successfully
            if flag.overridden_to.is_some() && flag.overridden_at.is_some() {
                flag.overridden_at = None;
                flag.overridden_to = None;
            }
        }
    }

    async fn refresh_unleash_flags(
        &self,
        ctx: &Context,
        api: &Session,
        mail_stash: &Stash<UserDb>,
        modify_time: UnixTimestamp,
    ) -> CoreContextResult<()> {
        let context = FeatureFlagsService::unleash_feature_flags_context(ctx).await;
        let response = api.get_unleash_feature_flags(context).await?;
        let mut tether = mail_stash.connection();
        let mut flags = Self::fetch_from_cache(&tether, UserFeatureFlagSource::Unleash).await;
        for flag in flags.values_mut() {
            // Unleash returns only enabled flags. We don't want to remove them from cache or keep stale data.
            // But instead, we are marking them with false.
            // The easiest way to do so is to mark everything as disabled and then in `set_flags_from_unleash` mark
            // every present flag as enabled.
            flag.enabled = false;
            flag.variant_name = None;
            flag.variant_enabled = None;
            flag.variant_payload_type = None;
            flag.variant_payload_value = None;
        }

        Self::set_flags_from_unleash(&mut flags, response, modify_time);

        let flags = flags.into_values().collect();

        tether
            .write_tx(async |tx| UserFeatureFlag::save_all(flags, tx).await)
            .await?;
        Ok(())
    }

    async fn refresh_legacy_flags(
        &self,
        api: &Session,
        mail_stash: &Stash<UserDb>,
        modify_time: UnixTimestamp,
    ) -> CoreContextResult<()> {
        let initial_flags = GetLegacyFeatureFlagsOptions {
            feature_type: Some(LegacyFeatureFlagType::Boolean),
            ..Default::default()
        };
        let response = PaginateLegacyFeatureFlags::fetch_all_filtered(api, initial_flags).await?;

        let mut tether = mail_stash.connection();
        let mut cached_flags = Self::fetch_from_cache(&tether, UserFeatureFlagSource::Legacy)
            .await
            .into_iter()
            .map(|(key, flag)| (key, (flag, FlagPersistence::DontPersist)))
            .collect::<BTreeMap<_, _>>();

        Self::set_flags_from_legacy(&mut cached_flags, response, modify_time);

        let mut flags_to_remove = Vec::new();
        let mut flags_to_save = Vec::new();

        for (name, (flag, persistence)) in cached_flags {
            match persistence {
                FlagPersistence::Persist => flags_to_save.push(flag),
                FlagPersistence::DontPersist => flags_to_remove.push(name),
            }
        }

        tether
            .write_tx(async |tx| {
                UserFeatureFlag::delete_batch_from_source(
                    flags_to_remove,
                    UserFeatureFlagSource::Legacy,
                    tx,
                )
                .await?;
                UserFeatureFlag::save_all(flags_to_save, tx).await
            })
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip_all, name = "UserFeatureFlagsRefresh")]
    pub async fn refresh(&self) -> CoreContextResult<()> {
        let ctx = self.ctx.upgrade().context("Could not upgrade context")?;
        let api = ctx.session();

        let modify_time = UnixTimestamp::now();

        let legacy_flags = self.refresh_legacy_flags(api, ctx.mail_stash(), modify_time);
        let unleash_flags =
            self.refresh_unleash_flags(&ctx.context, api, ctx.mail_stash(), modify_time);

        // We do not use `try_join` here because even if only one endpoint is working, we still want to
        // update those flags.
        let (legacy_flags, unleash_flags) = tokio::join!(legacy_flags, unleash_flags);

        if let Err(error) = legacy_flags {
            tracing::error!(%error, "Failed to refresh legacy flags");
        }
        if let Err(error) = unleash_flags {
            tracing::error!(%error, "Failed to refresh Unleash flags", );
        }

        Ok(())
    }

    pub async fn get(&self, key: &str) -> CoreContextResult<Option<bool>> {
        let ctx = self.ctx.upgrade().context("Could not upgrade context")?;
        let feature_flag = {
            let tether = ctx.mail_stash().connection();
            UserFeatureFlag::by_name(key, &tether).await?
        };
        Ok(feature_flag.map(|flag| flag.is_enabled()))
    }

    pub async fn get_feature_flag_variant(&self, key: &str) -> CoreContextResult<Option<Variant>> {
        let ctx = self.ctx.upgrade().context("Could not upgrade context")?;
        let feature_flag = {
            let tether = ctx.mail_stash().connection();
            UserFeatureFlag::by_name(key, &tether).await?
        };
        Ok(feature_flag.and_then(|flag| flag.variant()))
    }

    pub async fn list_all(&self) -> Vec<(String, bool)> {
        let Some(ctx) = self.ctx.upgrade() else {
            tracing::warn!("Failed to upgrade context");
            return vec![];
        };
        let tether = ctx.mail_stash().connection();
        let flags = UserFeatureFlag::all(&tether)
            .await
            .inspect_err(|err| tracing::warn!("Failed to fetch feature flags: {}", err))
            .unwrap_or_default();

        tracing::info!("Retrieved {} feature flags", flags.len());

        flags
            .iter()
            .map(|flag| (flag.name.clone(), flag.enabled))
            .collect()
    }

    pub async fn watch(&self) -> CoreContextResult<WatcherHandle> {
        let ctx = self.ctx.upgrade().context("Could not upgrade context")?;

        let mail_stash = ctx.mail_stash();
        TableWatcher::<UserFeatureFlag>::watch(mail_stash)
            .await
            .map_err(CoreContextError::from)
    }
}

struct PaginateLegacyFeatureFlags;

impl Paginatable for PaginateLegacyFeatureFlags {
    type PaginateOptions = GetLegacyFeatureFlagsOptions;

    type Response = GetLegacyFeaturesResponse;

    type Output = LegacyFeatureFlag;

    type Error = ApiServiceError;

    type API = Session;

    const NAME: &'static str = "Legacy Feature Flags";

    const DEFAULT_PAGE_SIZE: u64 = MAX_LEGACY_FEATURES_PER_PAGE;

    async fn fetch(
        api: &Self::API,
        options: Self::PaginateOptions,
    ) -> Result<Self::Response, Self::Error> {
        api.get_legacy_feature_flags(options).await
    }
}
