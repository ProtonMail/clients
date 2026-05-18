use crate::service::ApiServiceResult;
use crate::services::proton::DATA_V1;
use crate::services::proton::prelude::*;
use mail_muon::common::{Sender, ServiceType};
use mail_muon::util::ProtonRequestExt;
use mail_muon::{POST, ProtonRequest, ProtonResponse};

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
