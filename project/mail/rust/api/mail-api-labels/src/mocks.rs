use crate::{
    Label as ApiLabel, LabelId, LabelType, PostLabelsRequest, PostLabelsResponse, PutLabelRequest,
    PutLabelResponse,
};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

pub fn mock_create_label(name: String, color: String, res: Result<LabelId, u16>) -> Mock {
    let request = PostLabelsRequest {
        parent_id: None,
        color: color.clone(),
        label_type: LabelType::Label,
        name: name.clone(),
        notify: true,
    };
    let resp = match res {
        Ok(label_id) => ResponseTemplate::new(200).set_body_json(PostLabelsResponse {
            label: ApiLabel {
                id: label_id,
                parent_id: None,
                color,
                display: true,
                expanded: true,
                label_type: LabelType::Label,
                name,
                notify: true,
                order: 1,
                path: None,
                sticky: false,
                last_unseen_message: None,
            },
        }),
        Err(status_code) => ResponseTemplate::new(status_code),
    };

    Mock::given(method("POST"))
        .and(path("/api/core/v4/labels"))
        .and(body_json(request))
        .respond_with(resp)
}

pub fn mock_create_folder(
    parent_id: Option<LabelId>,
    name: String,
    color: String,
    notify: bool,
    res: Result<LabelId, u16>,
) -> Mock {
    let request = PostLabelsRequest {
        parent_id: parent_id.clone(),
        color: color.clone(),
        label_type: LabelType::Folder,
        name: name.clone(),
        notify,
    };
    let resp = match res {
        Ok(label_id) => ResponseTemplate::new(200).set_body_json(PostLabelsResponse {
            label: ApiLabel {
                id: label_id,
                parent_id,
                color,
                display: true,
                expanded: true,
                label_type: LabelType::Folder,
                name,
                notify,
                order: 1,
                path: None,
                sticky: false,
                last_unseen_message: None,
            },
        }),
        Err(status_code) => ResponseTemplate::new(status_code),
    };

    Mock::given(method("POST"))
        .and(path("/api/core/v4/labels"))
        .and(body_json(request))
        .respond_with(resp)
}

pub fn mock_put_label(
    label_id: LabelId,
    name: String,
    color: String,
    resp: Result<(), u16>,
) -> Mock {
    let request = PutLabelRequest {
        parent_id: None,
        color: color.clone(),
        name: name.clone(),
        notify: Some(false),
    };
    let resp = match resp {
        Ok(()) => ResponseTemplate::new(200).set_body_json(PutLabelResponse {
            label: ApiLabel {
                id: label_id.clone(),
                parent_id: None,
                color,
                display: true,
                expanded: true,
                label_type: LabelType::Label,
                name,
                notify: false,
                order: 1,
                path: None,
                sticky: false,
                last_unseen_message: None,
            },
        }),
        Err(status_code) => ResponseTemplate::new(status_code),
    };

    Mock::given(method("PUT"))
        .and(path(format!("/api/core/v4/labels/{label_id}")))
        .and(body_json(request))
        .respond_with(resp)
}

pub fn mock_put_folder(
    label_id: LabelId,
    parent_id: Option<LabelId>,
    name: String,
    color: String,
    notify: bool,
    resp: Result<(), u16>,
) -> Mock {
    let request = PutLabelRequest {
        parent_id: parent_id.clone(),
        color: color.clone(),
        name: name.clone(),
        notify: Some(notify),
    };

    let resp = match resp {
        Ok(()) => ResponseTemplate::new(200).set_body_json(PutLabelResponse {
            label: ApiLabel {
                id: label_id.clone(),
                parent_id,
                color,
                display: true,
                expanded: true,
                label_type: LabelType::Folder,
                name,
                notify,
                order: 1,
                path: None,
                sticky: false,
                last_unseen_message: None,
            },
        }),
        Err(status_code) => ResponseTemplate::new(status_code),
    };
    Mock::given(method("PUT"))
        .and(path(format!("/api/core/v4/labels/{label_id}")))
        .and(body_json(request))
        .respond_with(resp)
}

pub fn mock_delete_label(label_id: LabelId, status_code: u16) -> Mock {
    Mock::given(method("DELETE"))
        .and(path(format!("/api/core/v4/labels/{label_id}")))
        .respond_with(ResponseTemplate::new(status_code))
}
