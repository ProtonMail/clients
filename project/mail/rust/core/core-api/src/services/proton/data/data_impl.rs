use crate::service::ApiServiceResult;
use crate::services::proton::DATA_V1;
use crate::services::proton::prelude::*;
use mail_muon::POST;
use mail_muon::ProtonRequest;
use mail_muon::ProtonResponse;
use mail_muon::common::Sender;
use mail_muon::common::ServiceType;
use mail_muon::util::ProtonRequestExt;

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> ProtonData for This {
    async fn post_metrics(&self, metrics: Vec<PostMetricsRequestElement>) -> ApiServiceResult<()> {
        POST!("{DATA_V1}/metrics")
            .body_json(PostMetricsRequest { metrics })?
            .header(("Priority", "u=6"))
            .service_type(ServiceType::Background, true)
            .send_with(self)
            .await?
            .ok()?;
        Ok(())
    }
}
