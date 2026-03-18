use mail_muon::{POST, ProtonRequest, ProtonResponse, common::Sender, http::HttpReqExt};

use crate::service::ApiServiceResult;

use super::{
    GROWTH_V1, ProtonGrowth,
    requests::{PostMeasurementEventRequest, PostMeasurementEventsRequest},
    responses::PostMeasurementEventResponse,
};

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
