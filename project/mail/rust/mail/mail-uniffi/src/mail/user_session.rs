mod events;
mod images;
mod labels;

use crate::core::datatypes::{
    AccountDetails, ConnectionStatus, GetPaymentsPlansOptions, Id, NewSubscription,
    NewSubscriptionValues, PaymentMethod, PaymentReceipt, PaymentToken, PaymentsPlans,
    PaymentsStatus, Subscriptions, UpsellEligibility, User, UserSettings,
};
use crate::core::measurement::{MeasurementEventType, MeasurementValue};
use crate::errors::unexpected::UnexpectedError;
use crate::errors::{ActionError, ProtonError, UserSessionError, VoidSessionResult};
use crate::mail::state::MailUserContextPtr;
use crate::{
    AsyncLiveQueryCallback, WatchHandle, async_runtime, declare_live_query_tagger, uniffi_async,
    watch_table,
};
use futures::TryFutureExt;
use mail_account_api::login::state::want_qr_confirmation::process_target_device_qr_code;
use mail_account_uniffi::login::ProcessTargetDeviceQrError;
use mail_account_uniffi::password::PasswordFlow;
use mail_account_uniffi::password_validator::PasswordValidatorService;
use mail_common::models::Attachment;
use mail_common::{MailContextError, MailUserContext};
use mail_common::{ProtonMailError as RealProtonMailError, UpsellEligibilityService};
use mail_core_api::services::proton::ProtonAuth;
use mail_core_common::UserContext;
use mail_core_common::actions::user_feature_flags::OverrideFlag;
use mail_core_common::services::{GrowthService, PaymentsService};
use mail_muon::common::IntoDyn;
use mail_observability::PreLoginMetricRecorder;
use mail_stash::UserDb;
use mail_stash::stash::{Stash, WatcherHandle};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::error;

use super::datatypes::AttachmentMetadata;

#[derive(uniffi::Object)]
pub struct MailUserSession {
    ctx: MailUserContextPtr,
}

impl MailUserSession {
    pub(crate) fn new(ctx: MailUserContextPtr) -> Arc<Self> {
        Arc::new(Self { ctx })
    }

    #[cfg(feature = "foundation_search_lab_harness")]
    fn initialize_real_bodies_api_from_path_impl(
        config_path: &str,
    ) -> Result<(), UserSessionError> {
        use std::path::Path;

        let file = mail_search_perf::fixture_bodies::load_remote_fixture_config_from_path(
            Path::new(config_path),
        )
        .map_err(|e| {
            tracing::error!("Failed to load remote fixture config: {}", e);
            UserSessionError::Other(ProtonError::Unexpected(UnexpectedError::Network))
        })?;
        let config = file.chunked_bodies.ok_or_else(|| {
            tracing::error!("Fixture config has no chunked_bodies section");
            UserSessionError::Other(ProtonError::Unexpected(UnexpectedError::Network))
        })?;

        mail_search_perf::fixture_bodies::initialize_real_bodies_api(config).map_err(|e| {
            tracing::error!("Failed to initialize real bodies from HTTP: {}", e);
            UserSessionError::Other(ProtonError::Unexpected(UnexpectedError::Network))
        })
    }

    pub(crate) fn ptr(&self) -> MailUserContextPtr {
        self.ctx.clone()
    }

    pub(crate) fn ctx(&self) -> Result<Arc<MailUserContext>, ProtonError> {
        Ok(self.ctx.upgrade().ok_or(UnexpectedError::Internal)?)
    }

    pub(crate) fn take_ctx(&self) -> Result<Arc<MailUserContext>, ProtonError> {
        Ok(self.ctx.consume().ok_or(UnexpectedError::Internal)?)
    }

    pub(crate) fn user_stash(&self) -> Result<Stash<UserDb>, ProtonError> {
        Ok(self.ctx()?.user_stash().to_owned())
    }
}

#[uniffi_export]
impl MailUserSession {
    #[must_use]
    pub fn user_id(&self) -> Result<String, ProtonError> {
        Ok(self.ctx()?.user_id().to_owned().into_inner())
    }

    #[must_use]
    pub fn session_id(&self) -> Result<String, ProtonError> {
        Ok(self.ctx()?.session_id().to_owned().into_inner())
    }

    /// Log out a session and delete all user data.
    #[returns(VoidSessionResult)]
    #[tracing::instrument(skip_all)]
    pub async fn logout(&self) -> Result<(), UserSessionError> {
        let ctx = self.take_ctx()?;

        uniffi_async(async move {
            ctx.logout_and_delete_data()
                .map_err(RealProtonMailError::from)
                .await
        })
        .await
        .map_err(UserSessionError::from)
        .into()
    }
}

/// Historic load (optional `mail-historic-search-load` dependency).
#[cfg(feature = "foundation_search_historic_load")]
#[uniffi_export]
impl MailUserSession {
    /// Trigger historic load: fetch messages from server and queue for indexing/prefetch
    ///
    /// This is a debug/testing function that fetches messages from the server and queues them
    /// for indexing (if they have bodies) or prefetch (if they need bodies).
    ///
    /// # Arguments
    /// * `label_id` - Optional label ID string (defaults to All Mail if None)
    /// * `max_messages` - Optional maximum number of messages to process
    /// * `page_size` - Optional page size for fetching (default: 100)
    ///
    /// # Returns
    /// A `HistoricLoadResult` with counts of fetched, indexed, and prefetched messages
    pub async fn historic_load(
        &self,
        label_id: Option<String>,
        max_messages: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<crate::mail::datatypes::HistoricLoadResult, UserSessionError> {
        use mail_core_api::services::proton::LabelId as RealLabelId;
        use mail_historic_search_load::historic_load_messages;

        let ctx = self.ctx()?;

        let label_id = label_id.map(RealLabelId::from);
        let max_messages = max_messages.map(|n| usize::try_from(n).unwrap_or(usize::MAX));
        let page_size = page_size.map(|n| usize::try_from(n).unwrap_or(usize::MAX));

        uniffi_async(async move {
            let result = historic_load_messages(&ctx, label_id, max_messages, page_size)
                .await
                .map_err(RealProtonMailError::from)?;

            Result::<_, RealProtonMailError>::Ok(crate::mail::datatypes::HistoricLoadResult {
                messages_fetched: result.messages_fetched as u64,
                messages_indexed: result.messages_indexed as u64,
                messages_prefetched: result.messages_prefetched as u64,
            })
        })
        .await
        .map_err(UserSessionError::from)
    }
}

/// Real email bodies from S3 chunked bucket (replaces old fixture/Lambda approach)
#[cfg(feature = "foundation_search_lab_harness")]
#[uniffi_export]
impl MailUserSession {
    /// Start downloading real email bodies from the chunked HTTPS bucket.
    ///
    /// Reads `FIXTURE_REMOTE_CONFIG_PATH` (UTF-8 JSON with a `chunked_bodies` block — see
    /// `fixture_remote.config.example.json`), then downloads `_index.json` and chunk files.
    /// Bodies stay encrypted in memory; decryption runs lazily during prefetch.
    ///
    /// On Android, prefer [`Self::initialize_real_bodies_api_from_path`]: there is no shell
    /// environment for `FIXTURE_REMOTE_CONFIG_PATH`.
    pub fn initialize_real_bodies_api(&self) -> Result<(), UserSessionError> {
        use crate::errors::ProtonError;
        use crate::errors::unexpected::UnexpectedError;

        let path = std::env::var("FIXTURE_REMOTE_CONFIG_PATH").map_err(|_| {
            tracing::error!("FIXTURE_REMOTE_CONFIG_PATH is not set");
            UserSessionError::Other(ProtonError::Unexpected(UnexpectedError::Network))
        })?;
        Self::initialize_real_bodies_api_from_path_impl(&path)
    }

    /// Same as [`Self::initialize_real_bodies_api`], but reads the UTF-8 JSON from `config_path`
    /// (absolute path). Hosts: same shape as `fixture_remote.config.example.json` (`batch_api`
    /// optional, `chunked_bodies` required for S3/HTTPS bodies).
    pub fn initialize_real_bodies_api_from_path(
        &self,
        config_path: &str,
    ) -> Result<(), UserSessionError> {
        Self::initialize_real_bodies_api_from_path_impl(config_path)
    }

    /// Check if the real-bodies store has been initialized (loader started)
    #[must_use]
    pub fn is_real_bodies_initialized(&self) -> bool {
        mail_search_perf::fixture_bodies::is_real_bodies_initialized()
    }

    /// Check if all chunks have finished downloading
    #[must_use]
    pub fn is_real_bodies_loading_complete(&self) -> bool {
        mail_search_perf::fixture_bodies::is_real_bodies_loading_complete()
    }

    /// Get the number of real bodies currently loaded (encrypted, in memory)
    #[must_use]
    pub fn real_bodies_loaded(&self) -> u64 {
        mail_search_perf::fixture_bodies::real_bodies_loaded() as u64
    }

    /// Reset the fixture body index to start from the beginning.
    /// Call before historic_load to ensure bodies are served from the start.
    pub fn reset_fixture_bodies(&self) {
        mail_search_perf::fixture_bodies::reset_index();
    }
}

/// Database inspection methods for debugging
#[cfg(feature = "foundation_search")]
#[uniffi_export]
impl MailUserSession {
    /// Get search index blob statistics
    ///
    /// Returns a list of blob names and their sizes in bytes
    pub fn get_search_index_blobs(&self) -> Result<Vec<SearchIndexBlob>, UserSessionError> {
        let ctx = self.ctx()?;
        let stash = ctx.user_stash().clone();

        async_runtime().block_on(async {
            let tether = match stash.connection().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Failed to get DB connection: {:?}", e);
                    return Ok(vec![]);
                }
            };

            let result = tether
                .sync_query(|conn| {
                    let mut stmt = conn.prepare(
                        "SELECT blob_name, length(blob_data) as size FROM search_index_blobs ORDER BY size DESC",
                    )?;
                    let rows = stmt.query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                    })?;
                    let mut results = vec![];
                    for row in rows {
                        if let Ok((name, size)) = row {
                            results.push(SearchIndexBlob {
                                name,
                                size: size.cast_unsigned(),
                            });
                        }
                    }
                    Ok(results)
                })
                .await;

            match result {
                Ok(blobs) => Ok(blobs),
                Err(e) => {
                    tracing::error!("Failed to query search index blobs: {:?}", e);
                    Ok(vec![])
                }
            }
        })
    }

    /// Get sample message bodies from the database
    ///
    /// Returns the first N message bodies with their IDs and a preview
    pub fn get_message_body_samples(
        &self,
        limit: u32,
    ) -> Result<Vec<MessageBodySample>, UserSessionError> {
        let ctx = self.ctx()?;
        let stash = ctx.user_stash().clone();

        async_runtime().block_on(async {
            let tether = match stash.connection().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Failed to get DB connection: {:?}", e);
                    return Ok(vec![]);
                }
            };

            let result = tether
                .sync_query(move |conn| {
                    // body is BLOB, so we read it as Vec<u8> and convert to string
                    let mut stmt = conn.prepare(&format!(
                        "SELECT message_id, length(body), body FROM raw_message_body LIMIT {limit}"
                    ))?;
                    let rows = stmt.query_map([], |row| {
                        let id: i64 = row.get(0)?;
                        let size: i64 = row.get(1)?;
                        let body_bytes: Vec<u8> = row.get(2)?;
                        Ok((id, size, body_bytes))
                    })?;
                    let mut results = vec![];
                    for row in rows {
                        if let Ok((id, size, body_bytes)) = row {
                            // Try to convert body to string, take first 100 chars
                            let preview = if body_bytes.is_empty() {
                                "(empty)".to_string()
                            } else {
                                // Try UTF-8 first, fall back to lossy conversion
                                let text = String::from_utf8_lossy(&body_bytes);
                                let preview: String = text.chars().take(100).collect();
                                if preview.is_empty() {
                                    format!("(binary: {} bytes)", body_bytes.len())
                                } else {
                                    preview
                                }
                            };
                            results.push(MessageBodySample {
                                message_id: id.cast_unsigned(),
                                size: size.cast_unsigned(),
                                preview,
                            });
                        }
                    }
                    Ok(results)
                })
                .await;

            match result {
                Ok(samples) => Ok(samples),
                Err(e) => {
                    tracing::error!("Failed to query message bodies: {:?}", e);
                    Ok(vec![])
                }
            }
        })
    }

    /// Get total count of indexed messages
    pub fn get_indexed_message_count(&self) -> Result<u64, UserSessionError> {
        let ctx = self.ctx()?;
        let stash = ctx.user_stash().clone();

        async_runtime().block_on(async {
            let tether = match stash.connection().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Failed to get DB connection: {:?}", e);
                    return Ok(0);
                }
            };

            let result = tether
                .sync_query(|conn| {
                    Ok(conn.query_row(
                        "SELECT COUNT(*) FROM search_index_content_hashes",
                        [],
                        |row| row.get::<_, i64>(0),
                    )?)
                })
                .await;

            match result {
                Ok(count) => Ok(count.cast_unsigned()),
                Err(e) => {
                    tracing::error!("Failed to count indexed messages: {:?}", e);
                    Ok(0)
                }
            }
        })
    }

    /// Get total count of stored message bodies
    pub fn get_stored_body_count(&self) -> Result<u64, UserSessionError> {
        let ctx = self.ctx()?;
        let stash = ctx.user_stash().clone();

        async_runtime().block_on(async {
            let tether = match stash.connection().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Failed to get DB connection: {:?}", e);
                    return Ok(0);
                }
            };

            let result = tether
                .sync_query(|conn| {
                    Ok(
                        conn.query_row("SELECT COUNT(*) FROM raw_message_body", [], |row| {
                            row.get::<_, i64>(0)
                        })?,
                    )
                })
                .await;

            match result {
                Ok(count) => Ok(count.cast_unsigned()),
                Err(e) => {
                    tracing::error!("Failed to count message bodies: {:?}", e);
                    Ok(0)
                }
            }
        })
    }
}

/// Search index blob info
#[derive(Debug, Clone, uniffi::Record)]
pub struct SearchIndexBlob {
    pub name: String,
    pub size: u64,
}

/// Message body sample for debugging
#[derive(Debug, Clone, uniffi::Record)]
pub struct MessageBodySample {
    pub message_id: u64,
    pub size: u64,
    pub preview: String,
}

declare_live_query_tagger!(WatchAddressesMarker);
declare_live_query_tagger!(WatchUserMarker);
declare_live_query_tagger!(WatchUserSettingsMarker);
declare_live_query_tagger!(WatchUpsellEligibilityMarker);

#[uniffi_export]
impl MailUserSession {
    #[tracing::instrument(skip_all)]
    pub fn watch_addresses(
        &self,
        callback: Arc<dyn AsyncLiveQueryCallback>,
    ) -> Result<Arc<WatchHandle>, ProtonError> {
        let ctx = self.ctx()?;
        let uctx = ctx.user_context();
        async_runtime().block_on(async {
            watch_table!(
                WatchAddressesMarker,
                uctx,
                ctx,
                callback,
                UserContext::watch_addresses
            )
        })
    }

    #[tracing::instrument(skip_all)]
    pub fn watch_user(
        &self,
        callback: Arc<dyn AsyncLiveQueryCallback>,
    ) -> Result<Arc<WatchHandle>, ProtonError> {
        let ctx = self.ctx()?;
        let uctx = ctx.user_context();
        async_runtime().block_on(async {
            watch_table!(
                WatchUserMarker,
                uctx,
                ctx,
                callback,
                UserContext::watch_user
            )
        })
    }

    #[tracing::instrument(skip_all)]
    pub fn watch_user_settings(
        &self,
        callback: Arc<dyn AsyncLiveQueryCallback>,
    ) -> Result<Arc<WatchHandle>, ProtonError> {
        let ctx = self.ctx()?;
        let uctx = ctx.user_context();
        async_runtime().block_on(async {
            watch_table!(
                WatchUserSettingsMarker,
                uctx,
                ctx,
                callback,
                UserContext::watch_user_settings
            )
        })
    }

    #[tracing::instrument(skip_all)]
    pub fn watch_upsell_eligibility(
        &self,
        callback: Arc<dyn AsyncLiveQueryCallback>,
    ) -> Result<Arc<WatchHandle>, ProtonError> {
        let ctx = self.ctx()?;
        let service = ctx.get_service::<UpsellEligibilityService>();
        async_runtime().block_on(async {
            watch_table!(
                WatchUpsellEligibilityMarker,
                service,
                ctx,
                callback,
                UpsellEligibilityService::watch_upsell_eligibility
            )
        })
    }

    #[tracing::instrument(skip_all)]
    pub fn watch_user_stream(&self) -> Result<Arc<WatchUserStream>, ProtonError> {
        let ctx = self.ctx()?;
        async_runtime().block_on(async {
            let handle = ctx
                .user_context()
                .watch_user()
                .await
                .map_err(RealProtonMailError::from)?;
            Ok(Arc::new(WatchUserStream {
                handle,
                token: ctx.user_context().create_child_cancellation_token(),
            }))
        })
    }
}

#[derive(uniffi::Object)]
pub struct WatchUserStream {
    handle: WatcherHandle,
    token: CancellationToken,
}

#[uniffi_export]
impl WatchUserStream {
    #[tracing::instrument(skip_all)]
    pub async fn next_async(self: Arc<Self>) -> Result<(), ProtonError> {
        async_runtime()
            .spawn(async move {
                let future = self.handle.receiver.recv_async();
                self.token
                    .run_until_cancelled(future)
                    .await
                    .ok_or_else(|| RealProtonMailError::from(MailContextError::TaskCancelled))?
                    .map_err(|_| ProtonError::Unexpected(UnexpectedError::Internal))
            })
            .await
            .map_err(RealProtonMailError::from)??;
        Ok(())
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }
}

#[uniffi_export]
impl MailUserSession {
    #[tracing::instrument(skip_all)]
    pub async fn session_uuid(&self) -> Result<String, UserSessionError> {
        let ctx = self.ctx()?;

        uniffi_async(async move {
            let res = ctx.session().get_sessions_uuid().await?;

            Result::<_, RealProtonMailError>::Ok(res.uuid)
        })
        .await
        .map_err(UserSessionError::from)
    }

    /// Fork the current session for a child with the given platform and product.
    ///
    /// This call has to be made from a parent session, and forks the current
    /// logged-in user session in order to provide a new session for the same
    /// user.
    ///
    /// If successful, this will return the "Selector" string for the new
    /// session. The child must present an app version that matches the platform
    /// and product.
    ///
    #[tracing::instrument(skip_all)]
    pub async fn fork(
        &self,
        platform: String,
        product: String,
    ) -> Result<crate::mail::datatypes::Fork, UserSessionError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            ctx.session()
                .fork(platform, product)
                .await
                .map(Into::into)
                .map_err(RealProtonMailError::from)
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn new_password_change_flow(&self) -> Result<Arc<PasswordFlow>, UserSessionError> {
        let ctx = self.ctx()?;

        uniffi_async(async move {
            ctx.new_password_change_flow()
                .await
                .map(|flow| PasswordFlow::new(flow))
                .map_err(RealProtonMailError::from)
        })
        .await
        .map_err(UserSessionError::from)
    }

    /// Processes a QR code from a Target Device to initiate a secure session fork.
    ///
    /// This function parses the provided QR code, retrieves the current device's session passphrase,
    /// optionally encrypts it using the encryption key from the QR code, and sends a fork confirmation
    /// to the Target Device.
    pub async fn process_target_device_qr_code(
        &self,
        qr_code: String,
    ) -> Result<(), ProcessTargetDeviceQrError> {
        let ctx = self
            .ctx()
            .inspect_err(|err| error!("Failed to get Context: {err:?}"))
            .map_err(|_| {
                ProcessTargetDeviceQrError::Other(String::from("Failed to get Context"))
            })?;
        let session = ctx.session().to_owned();
        let (client, _) = session.into_parts();
        let core_context = Arc::clone(ctx.core_context());
        let observability = PreLoginMetricRecorder::default();
        uniffi_async::<_, ProcessTargetDeviceQrError, _>(async move {
            process_target_device_qr_code(&qr_code, client, core_context, observability)
                .await
                .map_err(ProcessTargetDeviceQrError::from)
        })
        .await
        .into()
    }

    #[must_use]
    pub fn password_validator(&self) -> Option<Arc<PasswordValidatorService>> {
        let ctx = self
            .ctx()
            .inspect_err(|err| error!("Failed to get Context: {err:?}"))
            .ok()?;
        Some(Arc::new(PasswordValidatorService::setup(
            ctx.session().to_owned().into_dyn(),
        )))
    }

    #[tracing::instrument(skip_all)]
    pub async fn user(&self) -> Result<User, UserSessionError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            let user = ctx.user().await?;
            Result::<_, RealProtonMailError>::Ok(user.into())
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn account_details(&self) -> Result<AccountDetails, UserSessionError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            let account_details = ctx.account_details().await?;
            Result::<_, RealProtonMailError>::Ok(account_details.into())
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn user_settings(&self) -> Result<UserSettings, UserSessionError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            let settings = ctx.user_settings().ok_into().await?;

            Result::<_, RealProtonMailError>::Ok(settings)
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn record_measurement(
        &self,
        event_type: MeasurementEventType,
        asid: String,
        app_package_name: String,
        fields: HashMap<String, Option<MeasurementValue>>,
    ) -> Result<(), UserSessionError> {
        let ctx = self.ctx()?;
        let new_session = matches!(event_type, MeasurementEventType::Open { new_session: true });

        uniffi_async(async move {
            let service = ctx.user_context().get_service::<GrowthService>();

            if new_session {
                service.clear_session_start();
            }

            let fields = fields
                .into_iter()
                .map(|(k, v)| (k, v.map(Into::into)))
                .collect();
            service
                .record(event_type.into(), asid, app_package_name, fields)
                .await
                .map_err(|e| RealProtonMailError::from(MailContextError::from(e)))
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_attachment(
        &self,
        local_attachment_id: Id,
    ) -> Result<DecryptedAttachment, ActionError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            let mut tether = ctx.user_stash().connection().await?;
            Attachment::get_attachment(&ctx, local_attachment_id.into(), &mut tether)
                .await
                .map(DecryptedAttachment::try_from)?
                .map_err(RealProtonMailError::from)
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument(skip_all)]
    pub async fn connection_status(&self) -> Result<ConnectionStatus, UserSessionError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            let status = ctx.connection_status().into();
            // unfortunatelly there is join error here which need to be handled
            Result::<ConnectionStatus, RealProtonMailError>::Ok(status)
        })
        .await
        .map_err(UserSessionError::from)
    }

    /// Execute callback when connection status is online
    ///
    /// The method will execute callback immediately when current status is online
    /// otherwise it will wait till the status is online again and then execute callback
    ///
    pub fn execute_when_online(&self, callback: Arc<dyn ExecuteWhenOnlineCallback>) {
        let Ok(ctx) = self.ctx() else {
            tracing::error!("Cannot obtain context, callback will not be executed");
            return;
        };

        ctx.spawn_ex(async move |ctx| {
            ctx.network_monitor_service()
                .network_status_observer()
                .wait_until_online()
                .await;

            _ = async_runtime()
                .spawn_blocking(move || {
                    callback.on_online();
                })
                .await;
        });
    }

    /// Execute callback when connection status is online
    ///
    /// The method will execute callback immediately when current status is online
    /// otherwise it will wait till the status is online again and then execute callback
    ///
    pub fn execute_when_online_async(&self, callback: Arc<dyn ExecuteWhenOnlineCallbackAsync>) {
        let Ok(ctx) = self.ctx() else {
            tracing::error!("Cannot obtain context, callback will not be executed");
            return;
        };

        ctx.spawn_ex(async move |ctx| {
            ctx.network_monitor_service()
                .network_status_observer()
                .wait_until_online()
                .await;

            callback.on_online().await;
        });
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_payments_status(
        &self,
        vendor: String,
    ) -> Result<PaymentsStatus, UserSessionError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            let res = ctx
                .user_context()
                .get_service::<PaymentsService>()
                .get_payments_status(vendor)
                .await
                .map_err(MailContextError::from)?;

            Result::<_, RealProtonMailError>::Ok(PaymentsStatus {
                location: res.location.into(),
                payment_methods: res.payment_methods.into(),
            })
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_payments_plans(
        &self,
        options: GetPaymentsPlansOptions,
    ) -> Result<PaymentsPlans, UserSessionError> {
        let ctx = self.ctx()?;

        uniffi_async(async move {
            let res = ctx
                .user_context()
                .get_service::<PaymentsService>()
                .get_payments_plans(options.into())
                .await
                .map_err(MailContextError::from)?;

            Result::<_, RealProtonMailError>::Ok(PaymentsPlans {
                plans: res.plans.into_iter().map(Into::into).collect(),
                default_cycle: res.default_cycle.into(),
            })
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_payments_resources_icons(
        &self,
        name: String,
    ) -> Result<Vec<u8>, UserSessionError> {
        let ctx = self.ctx()?;

        uniffi_async(async move {
            let res = ctx
                .user_context()
                .get_service::<PaymentsService>()
                .get_payments_resources_icons(name)
                .await
                .map_err(MailContextError::from)?;

            Result::<_, RealProtonMailError>::Ok(res.into())
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn post_payments_tokens(
        &self,
        amount: u64,
        currency: String,
        payment: PaymentReceipt,
    ) -> Result<PaymentToken, UserSessionError> {
        let ctx = self.ctx()?;

        uniffi_async(async move {
            let res = ctx
                .user_context()
                .get_service::<PaymentsService>()
                .post_payments_tokens(amount, currency, payment.into())
                .await
                .map_err(MailContextError::from)?;

            Result::<_, RealProtonMailError>::Ok(PaymentToken {
                token: res.token,
                status: res.status,
            })
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_payments_subscription(&self) -> Result<Subscriptions, UserSessionError> {
        let ctx = self.ctx()?;

        uniffi_async(async move {
            let res = ctx
                .user_context()
                .get_service::<PaymentsService>()
                .get_payments_subscription()
                .await
                .map_err(MailContextError::from)?;
            let current = res.subscriptions.into_iter().map(Into::into);
            let upcoming = res.upcoming_subscriptions.into_iter().map(Into::into);

            Result::<_, RealProtonMailError>::Ok(Subscriptions {
                current: current.collect(),
                upcoming: upcoming.collect(),
            })
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn post_payments_subscription(
        &self,
        subscription: NewSubscription,
        new_values: NewSubscriptionValues,
    ) -> Result<(), UserSessionError> {
        let ctx = self.ctx()?;

        uniffi_async(async move {
            ctx.user_context()
                .get_service::<PaymentsService>()
                .post_payments_subscription(subscription.into(), new_values.into())
                .await
                .map_err(MailContextError::from)?;

            Result::<_, RealProtonMailError>::Ok(())
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_payment_method(&self, id: String) -> Result<PaymentMethod, UserSessionError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            let res = ctx
                .user_context()
                .get_service::<PaymentsService>()
                .get_payment_method(id)
                .await
                .map_err(MailContextError::from)?;

            Ok::<_, RealProtonMailError>(res.payment_method.into())
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn upsell_eligibility(&self) -> Result<UpsellEligibility, UserSessionError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            let service = ctx.get_service::<UpsellEligibilityService>();
            let upsell_eligibility = service.upsell_eligibility().await?;
            Result::<_, RealProtonMailError>::Ok(upsell_eligibility.into())
        })
        .await
        .map_err(UserSessionError::from)
    }

    #[tracing::instrument(skip_all)]
    pub async fn has_valid_sender_address(&self) -> Result<bool, ProtonError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            let address = ctx
                .user_context()
                .address_service()
                .find_valid_sender_address()
                .await
                .map_err(MailContextError::from)?;
            Result::<_, RealProtonMailError>::Ok(address.is_some())
        })
        .await
        .map_err(ProtonError::from)
    }

    /// Is the Unleash OR legacy feature enabled. Returns not only global feature flags,
    /// but also user-specific ones.
    ///
    /// If you don't have an active user session, use [`MailSession::is_feature_enabled`] instead.
    ///
    /// Currently:
    /// * Returns None if feature is not found
    /// * Returns Some(true) if feature is enabled (or present in case of Unleash)
    /// * Returns Some(false) if feature is disabled (only legacy, Unleash returns None in that case)
    ///
    /// If there are two features with the same id, coming from unleash and legacy, unleash takes the precedence.
    #[tracing::instrument(skip_all)]
    pub async fn is_feature_enabled(
        &self,
        feature_id: String,
    ) -> Result<Option<bool>, ProtonError> {
        let ctx = self.ctx()?;

        uniffi_async(async move {
            let flag = ctx
                .user_context()
                .feature_flags()
                .get(&feature_id)
                .await
                .map_err(MailContextError::from)?;

            Ok::<_, RealProtonMailError>(flag)
        })
        .await
        .map_err(ProtonError::from)
        .into()
    }

    #[tracing::instrument(skip_all)]
    pub fn watch_feature_flags_stream(
        &self,
    ) -> Result<Arc<WatchUserFeatureFlagsStream>, ProtonError> {
        let ctx = self.ctx()?;

        async_runtime().block_on(async {
            let handle = ctx
                .user_context()
                .feature_flags()
                .watch()
                .await
                .map_err(MailContextError::from)
                .map_err(RealProtonMailError::from)?;
            Ok(Arc::new(WatchUserFeatureFlagsStream {
                handle,
                token: ctx.user_context().create_child_cancellation_token(),
            }))
        })
    }

    /// Fails if:
    /// * Feature is missing
    /// * Feature is not writable
    ///     * All Unleash flags are not writable.
    #[tracing::instrument(skip_all)]
    pub async fn override_user_feature_flag(
        &self,
        flag_name: String,
        new_value: bool,
    ) -> Result<(), ActionError> {
        let ctx = self.ctx()?;

        uniffi_async(async move {
            let action = OverrideFlag::new(flag_name, new_value);
            ctx.user_context()
                .queue()
                .queue_action(action)
                .await
                .map_err(|e| {
                    RealProtonMailError::Unexpected(
                        anyhow::anyhow!("Feature flag override failed: {e}").into(),
                    )
                })?;
            Ok::<_, RealProtonMailError>(())
        })
        .await
        .map_err(ActionError::from)
    }
}

impl TryFrom<mail_common::DecryptedAttachment> for DecryptedAttachment {
    type Error = anyhow::Error;

    fn try_from(value: mail_common::DecryptedAttachment) -> Result<Self, Self::Error> {
        let data_path = value
            .data_path
            .into_os_string()
            .into_string()
            .map_err(|str| anyhow::anyhow!("Path contained invalid utf8: {str:?}"))?;

        Ok(DecryptedAttachment {
            attachment_metadata: value.attachment_metadata.into(),
            data_path,
        })
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DecryptedAttachment {
    pub attachment_metadata: AttachmentMetadata,
    pub data_path: String,
}

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait ExecuteWhenOnlineCallbackAsync: Send + Sync {
    async fn on_online(&self);
}

#[uniffi::export(with_foreign)]
pub trait ExecuteWhenOnlineCallback: Send + Sync {
    fn on_online(&self);
}

#[derive(uniffi::Object)]
pub struct WatchUserFeatureFlagsStream {
    pub handle: WatcherHandle,
    token: CancellationToken,
}

#[uniffi_export]
impl WatchUserFeatureFlagsStream {
    #[tracing::instrument(skip_all)]
    pub async fn next_async(self: Arc<Self>) -> Result<(), ProtonError> {
        async_runtime()
            .spawn(async move {
                let future = self.handle.receiver.recv_async();
                self.token
                    .run_until_cancelled(future)
                    .await
                    .ok_or_else(|| RealProtonMailError::from(MailContextError::TaskCancelled))?
                    .map_err(|_| ProtonError::Unexpected(UnexpectedError::Internal))
            })
            .await
            .map_err(RealProtonMailError::from)??;
        Ok(())
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }
}
