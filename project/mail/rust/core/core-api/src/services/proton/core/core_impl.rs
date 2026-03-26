use bytes::Bytes;
use mail_muon::common::Sender;
use mail_muon::util::ProtonRequestExt;
use mail_muon::{GET, ProtonRequest, ProtonResponse, serde_to_query};
use serde_json::json;

use crate::service::ApiServiceResult;
use crate::services::proton::core::{
    CORE_V4, CORE_V5, CORE_V6, GetEventOptions, GetEventsLatestResponse, GetImagesLogoOptions,
    GetProxyImageResponse, ProtonCore,
};
use crate::services::proton::prelude::*;
use crate::utils::HeadersExt;

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> ProtonCore for This {
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
