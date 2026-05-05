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
pub use mail_api_bug_report::BugReportApi;
pub use mail_api_device::DeviceApi;
pub use mail_api_feature_flags::FeatureFlagsApi;
use mail_api_labels::LabelApi;
pub use mail_api_ping::PingApi;
use mail_contacts_api::ContactApi;

/// The Proton Core API base path (v4).
pub const CORE_V4: &str = "/core/v4";

/// The Proton Core API base path (v5).
pub const CORE_V5: &str = "/core/v5";

pub const CORE_V6: &str = "/core/v6";

/// Re-export Unleash API base path from mail-api-feature-flags.
pub use mail_api_feature_flags::UNLEASH_V2;

pub use mail_account_api::protocol::proton::ProtonAccount;

#[allow(async_fn_in_trait)]
pub trait ProtonCore:
    ProtonAccount + ContactApi + LabelApi + FeatureFlagsApi + DeviceApi + BugReportApi + PingApi
{
    async fn get_event(
        &self,
        event_id: EventId,
        options: GetEventOptions,
    ) -> ApiServiceResult<String>;

    async fn get_events_latest(&self) -> ApiServiceResult<GetEventsLatestResponse>;

    async fn get_images_logo(&self, options: GetImagesLogoOptions) -> ApiServiceResult<Bytes>;

    async fn proxy_img(
        &self,
        url: &url::Url,
        dry_run: bool,
    ) -> ApiServiceResult<GetProxyImageResponse>;

    async fn get_core_event_v6(&self, event_id: EventId) -> ApiServiceResult<String>;

    async fn get_core_event_latest_v6(&self) -> ApiServiceResult<GetEventsLatestResponse>;
}
