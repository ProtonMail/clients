mod common;
mod core_impl;
mod request_data;
mod requests;
mod response_data;
mod responses;

pub use self::common::*;
pub use self::request_data::*;
pub use self::requests::*;
pub use self::response_data::*;
pub use self::responses::*;
use crate::service::ApiServiceResult;
use bytes::Bytes;
use mail_api_labels::LabelApi;
use mail_muon::common::RetryPolicy;
use proton_crypto_account::keys::APIPublicAddressKeys;
use std::future::Future;
use std::time::Duration;

/// The Proton Core API base path (v4).
pub const CORE_V4: &str = "/core/v4";

/// The Proton Core API base path (v5).
pub const CORE_V5: &str = "/core/v5";

pub const CORE_V6: &str = "/core/v6";
pub const CONTACTS_V6: &str = "/contacts/v6";

/// Re-export Unleash API base path from core-feature-flags.
pub use core_feature_flags::UNLEASH_V2;

#[allow(async_fn_in_trait)]
pub trait ProtonCore: LabelApi {
    async fn get_addresses(&self) -> ApiServiceResult<GetAddressesResponse>;

    async fn get_address_by_id(&self, id: AddressId) -> ApiServiceResult<GetAddressResponse>;

    async fn get_captcha(&self, options: GetCaptchaOptions) -> ApiServiceResult<String>;

    /// GETs a single contact.
    ///
    /// This returns the full contact record.
    async fn get_contact(&self, contact_id: ContactId) -> ApiServiceResult<GetContactResponse>;

    /// GETs a list of contacts.
    ///
    /// This returns basic information — not the full contact record.
    async fn get_contacts(
        &self,
        options: GetContactsOptions,
    ) -> ApiServiceResult<GetContactsResponse>;

    /// GETs a list of emails for contacts.
    ///
    /// This returns basic information — not the full contact record.
    async fn get_contacts_emails(
        &self,
        options: GetContactsEmailsOptions,
    ) -> ApiServiceResult<GetContactsEmailsResponse>;

    async fn get_event(
        &self,
        event_id: EventId,
        options: GetEventOptions,
    ) -> ApiServiceResult<String>;

    async fn get_events_latest(&self) -> ApiServiceResult<GetEventsLatestResponse>;

    async fn get_images_logo(&self, options: GetImagesLogoOptions) -> ApiServiceResult<Bytes>;

    async fn get_keys_all(
        &self,
        options: GetKeysAllOptions,
    ) -> ApiServiceResult<APIPublicAddressKeys>;

    async fn get_keys_salts(&self) -> ApiServiceResult<GetKeysSaltsResponse>;

    async fn get_settings(&self) -> ApiServiceResult<GetSettingsResponse>;

    fn get_tests_ping(
        &self,
        timeout: Option<Duration>,
        retry: Option<RetryPolicy>,
    ) -> impl Future<Output = ApiServiceResult<()>> + Send;

    async fn get_users(&self) -> ApiServiceResult<GetUsersResponse>;

    async fn put_delete_contacts(
        &self,
        ids: Vec<ContactId>,
    ) -> ApiServiceResult<PutDeleteContactsResponse>;

    /// This method is used to register device for push notifications.
    /// The registering will delete any duplicate having the same (User ID, Product, Device Token) from different sessions.
    /// If the registering is done from a session already having a registered device, the existing device will be replaced with the new one.
    async fn register_device(&self, body: RegisterDeviceRequest) -> ApiServiceResult<()>;

    /// This method allows to create a ticket for bug in API (and in zendesk)
    /// for support team to review issue reported by a user.
    async fn post_report_bug(&self, body: PostReportBug) -> ApiServiceResult<()>;

    /// Gets an image through proton's proxy.
    /// When dry run is enabled, image is not really fetched from the remote server,
    /// but the information whether it is tracker or not is still returned.
    async fn proxy_img(
        &self,
        url: &url::Url,
        dry_run: bool,
    ) -> ApiServiceResult<GetProxyImageResponse>;

    /// Gets feature flags defined in Unleash service.
    /// See: <https://docs.getunleash.io/reference/api/unleash/get-frontend-features/>
    async fn get_unleash_feature_flags(&self) -> ApiServiceResult<GetUnleashFeaturesResponse>;

    /// Gets feature flags defined in our own legacy service.
    async fn get_legacy_feature_flags(
        &self,
        options: GetLegacyFeatureFlagsOptions,
    ) -> ApiServiceResult<GetLegacyFeaturesResponse>;

    /// Override a legacy feature flag value.
    async fn put_feature_flag_override(
        &self,
        flag_name: &str,
        new_value: bool,
    ) -> ApiServiceResult<PutFeatureFlagOverrideResponse>;

    async fn get_contact_event_v6(&self, event_id: EventId) -> ApiServiceResult<String>;

    async fn get_contact_event_latest_v6(&self) -> ApiServiceResult<GetEventsLatestResponse>;

    async fn get_core_event_v6(&self, event_id: EventId) -> ApiServiceResult<String>;

    async fn get_core_event_latest_v6(&self) -> ApiServiceResult<GetEventsLatestResponse>;
}
