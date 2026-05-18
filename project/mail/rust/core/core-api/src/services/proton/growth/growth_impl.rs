use mail_muon::common::Sender;
use mail_muon::http::HttpReqExt;
use mail_muon::{POST, ProtonRequest, ProtonResponse};

use crate::service::ApiServiceResult;

use super::requests::{PostMeasurementEventRequest, PostMeasurementEventsRequest};
use super::responses::PostMeasurementEventResponse;
use super::{GROWTH_V1, ProtonGrowth};

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> ProtonGrowth for This {
    async fn post_measurement(
        &self,
        request: PostMeasurementEventRequest,
    ) -> ApiServiceResult<PostMeasurementEventResponse> {
        Ok(POST!("{GROWTH_V1}/measurement")
            .body_json(request)?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn post_measurements(
        &self,
        request: PostMeasurementEventsRequest,
    ) -> ApiServiceResult<PostMeasurementEventResponse> {
        Ok(POST!("{GROWTH_V1}/measurements")
            .body_json(request)?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }
}
