use std::io::Cursor;
use std::time::Duration;

use bytes::Bytes;
use core_feature_flags::GetUnleashFeaturesContext;
use mail_muon::common::{RetryPolicy, Sender};
use mail_muon::util::ProtonRequestExt;
use mail_muon::{GET, POST, PUT};
use mail_muon::{ProtonRequest, ProtonResponse, serde_to_query};
use proton_crypto_account::keys::APIPublicAddressKeys;
use serde_json::json;

use crate::service::ApiServiceResult;
use crate::services::proton::core::{CORE_V4, CORE_V5, ProtonCore};
use crate::services::proton::prelude::*;
use crate::utils::{HeadersExt, HttpReqExt};

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> ProtonCore for This {
    async fn get_addresses(&self) -> ApiServiceResult<GetAddressesResponse> {
        Ok(GET!("{CORE_V4}/addresses")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_address_by_id(&self, id: AddressId) -> ApiServiceResult<GetAddressResponse> {
        Ok(GET!("{CORE_V4}/addresses/{id}")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_captcha(&self, options: GetCaptchaOptions) -> ApiServiceResult<String> {
        Ok(GET!("{CORE_V4}/captcha")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_string()?)
    }

    // Event APIs
    // https://protonmail.gitlab-pages.protontech.ch/Slim-API/core/#tag/Events

    async fn get_event(
        &self,
        event_id: EventId,
        options: GetEventOptions,
    ) -> ApiServiceResult<String> {
        Ok(GET!("{CORE_V5}/events/{event_id}")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_string()?)
    }

    async fn get_events_latest(&self) -> ApiServiceResult<GetEventsLatestResponse> {
        Ok(GET!("{CORE_V4}/events/latest")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_images_logo(&self, options: GetImagesLogoOptions) -> ApiServiceResult<Bytes> {
        Ok(GET!("{CORE_V4}/images/logo")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body()
            .into())
    }

    async fn get_keys_all(
        &self,
        options: GetKeysAllOptions,
    ) -> ApiServiceResult<APIPublicAddressKeys> {
        Ok(GET!("{CORE_V4}/keys/all")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_keys_salts(&self) -> ApiServiceResult<GetKeysSaltsResponse> {
        Ok(GET!("{CORE_V4}/keys/salts")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_settings(&self) -> ApiServiceResult<GetSettingsResponse> {
        Ok(GET!("{CORE_V4}/settings")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_tests_ping(
        &self,
        timeout: Option<Duration>,
        retry: Option<RetryPolicy>,
    ) -> ApiServiceResult<()> {
        GET!("{CORE_V4}/tests/ping")
            .with_allowed_time(timeout)
            .with_retry_policy(retry)
            .send_with(self)
            .await?
            .ok()?;

        Ok(())
    }

    async fn get_users(&self) -> ApiServiceResult<GetUsersResponse> {
        Ok(GET!("{CORE_V4}/users")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn register_device(&self, body: RegisterDeviceRequest) -> ApiServiceResult<()> {
        POST!("{CORE_V4}/devices")
            .body_json(body)?
            .send_with(self)
            .await?
            .ok()?;

        Ok(())
    }

    async fn post_report_bug(&self, body: PostReportBug) -> ApiServiceResult<()> {
        POST!("{CORE_V4}/reports/bug")
            .multipart(move |mut form| {
                form.add_text("OS", body.os);
                form.add_text("OSVersion", body.os_version);
                form.add_text("Client", body.client);
                form.add_text("ClientVersion", body.client_version);
                form.add_text("ClientType", body.client_type.to_string());
                form.add_text("Title", body.title);
                form.add_text("Description", body.description);
                form.add_text("Username", body.username);
                form.add_text("Email", body.email);
                if let Some((file_name, logs)) = body.logs {
                    form.add_reader_file_with_mime(
                        "ApplicationLogs",
                        Cursor::new(logs),
                        file_name,
                        "application/zip"
                            .parse()
                            .expect("Somehow application/zip MIME left the earth"),
                    );
                }
                form
            })
            .await?
            .send_with(self)
            .await?
            .ok()?;

        Ok(())
    }

    async fn proxy_img(
        &self,
        url: &url::Url,
        dry_run: bool,
    ) -> ApiServiceResult<GetProxyImageResponse> {
        let query = json! ({
            "Url": url,
            "DryRun": i32::from(dry_run)
        });

        let response = GET!("{CORE_V4}/images")
            .query(serde_to_query(query)?)
            .send_with(self)
            .await?
            .ok()?;

        let headers = response.headers();
        let content_type = headers.get_string("Content-Type");
        let tracker_provider = headers.get_string("X-Pm-Tracker-Provider");

        let image = response.into_body();

        Ok(GetProxyImageResponse {
            image,
            content_type,
            tracker_provider,
        })
    }

    async fn get_unleash_feature_flags(
        &self,
        context: Option<GetUnleashFeaturesContext>,
    ) -> ApiServiceResult<GetUnleashFeaturesResponse> {
        let mut req = GET!("{UNLEASH_V2}/frontend");
        if let Some(ctx) = context {
            req = req.query(serde_to_query(ctx)?);
        }
        Ok(req.send_with(self).await?.ok()?.into_body_json()?)
    }

    async fn get_legacy_feature_flags(
        &self,
        options: GetLegacyFeatureFlagsOptions,
    ) -> ApiServiceResult<GetLegacyFeaturesResponse> {
        Ok(GET!("{CORE_V4}/features")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn put_feature_flag_override(
        &self,
        flag_name: &str,
        new_value: bool,
    ) -> ApiServiceResult<PutFeatureFlagOverrideResponse> {
        let request = PutFeatureFlagOverride { value: new_value };

        let response = PUT!("{CORE_V4}/features/{flag_name}/value")
            .body_json(request)?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?;

        Ok(response)
    }

    async fn get_core_event_v6(&self, event_id: EventId) -> ApiServiceResult<String> {
        Ok(GET!("{CORE_V6}/events/{event_id}")
            .send_with(self)
            .await?
            .ok()?
            .into_body_string()?)
    }

    async fn get_core_event_latest_v6(&self) -> ApiServiceResult<GetEventsLatestResponse> {
        Ok(GET!("{CORE_V6}/events/latest")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }
}
