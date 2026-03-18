use requests::{PostMeasurementEventRequest, PostMeasurementEventsRequest};
use responses::PostMeasurementEventResponse;

use crate::service::ApiServiceResult;

mod growth_impl;
mod request_data;
pub mod requests;
pub mod responses;

pub use self::request_data::*;

pub const GROWTH_V1: &str = "/growth/v1";

#[allow(async_fn_in_trait)]
pub trait ProtonGrowth {
    /// NOTE: This endpoint is made solely for Android
    async fn post_measurement(
        &self,
        request: PostMeasurementEventRequest,
    ) -> ApiServiceResult<PostMeasurementEventResponse>;

    /// NOTE: This endpoint is made solely for Android
    async fn post_measurements(
        &self,
        request: PostMeasurementEventsRequest,
    ) -> ApiServiceResult<PostMeasurementEventResponse>;
}
