//! Label API trait.

use mail_api_shared::ApiServiceResult;
use mail_muon::common::Sender;
use mail_muon::http::HttpReqExt;
use mail_muon::{DELETE, GET, PATCH, POST, PUT, ProtonRequest, ProtonResponse, serde_to_query};

use crate::{
    GetLabelsByIdsOptions, GetLabelsOptions, GetLabelsResponse, LabelId, LabelType,
    PatchLabelRequest, PatchLabelResponse, PostLabelsRequest, PostLabelsResponse, PutLabelRequest,
    PutLabelResponse,
};

const CORE_V4: &str = "/core/v4";

#[allow(async_fn_in_trait)]
pub trait LabelApi {
    async fn delete_label(&self, label_id: LabelId) -> ApiServiceResult<()>;

    async fn get_labels(&self, label_type: LabelType) -> ApiServiceResult<GetLabelsResponse>;

    /// Method to get labels by their IDs.
    /// Makes a POST request to the `/labels/by-ids` endpoint.
    /// Names refer to the fact labels are acquired by their IDs.
    /// HTTP `GET` method is not suppose to have a body,
    /// so POST method is used instead.
    async fn get_labels_by_ids(
        &self,
        label_ids: Vec<LabelId>,
    ) -> ApiServiceResult<GetLabelsResponse>;

    async fn post_labels(&self, body: PostLabelsRequest) -> ApiServiceResult<PostLabelsResponse>;

    async fn put_label(
        &self,
        label_id: LabelId,
        body: PutLabelRequest,
    ) -> ApiServiceResult<PutLabelResponse>;

    /// This method is used to patch an existing label.
    /// The `label_id` is used to identify the label to patch.
    /// Body contains expanded and notify fields.
    /// Expanded is a boolean that indicates if the label is expanded.
    /// For example if the folder is expanded in the UI.
    /// Notify is a boolean that indicates if the user should be notified
    /// about new messages in the label. By default both of them are disabled.
    async fn patch_label(
        &self,
        label_id: LabelId,
        body: PatchLabelRequest,
    ) -> ApiServiceResult<PatchLabelResponse>;
}

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> LabelApi for This {
    async fn delete_label(&self, label_id: LabelId) -> ApiServiceResult<()> {
        DELETE!("{CORE_V4}/labels/{label_id}")
            .send_with(self)
            .await?
            .ok()?;

        Ok(())
    }

    async fn get_labels(&self, label_type: LabelType) -> ApiServiceResult<GetLabelsResponse> {
        assert!(
            label_type != LabelType::ContactGroup,
            "Used dedicated contact api crate for this request"
        );
        Ok(GET!("{CORE_V4}/labels")
            .query(serde_to_query(GetLabelsOptions { label_type })?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_labels_by_ids(
        &self,
        label_ids: Vec<LabelId>,
    ) -> ApiServiceResult<GetLabelsResponse> {
        Ok(POST!("{CORE_V4}/labels/by-ids")
            .body_json(GetLabelsByIdsOptions { label_ids })?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn post_labels(&self, body: PostLabelsRequest) -> ApiServiceResult<PostLabelsResponse> {
        assert!(
            body.label_type != LabelType::ContactGroup,
            "Used dedicated contact api crate for this request"
        );
        Ok(POST!("{CORE_V4}/labels")
            .body_json(body)?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn put_label(
        &self,
        label_id: LabelId,
        body: PutLabelRequest,
    ) -> ApiServiceResult<PutLabelResponse> {
        Ok(PUT!("{CORE_V4}/labels/{label_id}")
            .body_json(body)?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn patch_label(
        &self,
        label_id: LabelId,
        body: PatchLabelRequest,
    ) -> ApiServiceResult<PatchLabelResponse> {
        Ok(PATCH!("{CORE_V4}/labels/{label_id}")
            .body_json(body)?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }
}
