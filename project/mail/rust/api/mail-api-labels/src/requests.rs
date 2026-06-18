//! Label API request structures.

use serde::Serialize;
use serde_with::{BoolFromInt, DefaultOnNull, serde_as};

use crate::{LabelId, LabelType};

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PostLabelsRequest {
    #[serde(rename = "ParentID")]
    pub parent_id: Option<LabelId>,

    pub color: String,

    #[serde(rename = "Type")]
    pub label_type: LabelType,

    pub name: String,

    #[serde_as(as = "DefaultOnNull<BoolFromInt>")]
    pub notify: bool,
}

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PutLabelRequest {
    #[serde(rename = "ParentID")]
    pub parent_id: Option<LabelId>,

    pub color: String,

    pub name: String,

    #[serde_as(as = "Option<BoolFromInt>")]
    pub notify: Option<bool>,
}

#[serde_as]
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PatchLabelRequest {
    #[serde_as(as = "Option<BoolFromInt>")]
    pub expanded: Option<bool>,
    #[serde_as(as = "Option<BoolFromInt>")]
    pub notify: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetLabelsOptions {
    #[serde(rename = "Type")]
    pub label_type: LabelType,
}

/// Represents `POST /labels/by-ids` request body.
///
/// Name refers to the fact it actually gets labels by their IDs.
/// But due to the fact GET requests are not supposed to have a body
/// The struct is used with the POST method instead.
///
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetLabelsByIdsOptions {
    /// Label IDs to get.
    #[serde(rename = "LabelIDs")]
    pub label_ids: Vec<LabelId>,
}
