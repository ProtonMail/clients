//! Shared API label types.

mod api;
#[cfg(feature = "mocks")]
pub mod mocks;
mod requests;
mod responses;

pub use api::LabelApi;
pub use requests::{
    GetLabelsByIdsOptions, GetLabelsOptions, PatchLabelRequest, PostLabelsRequest, PutLabelRequest,
};
pub use responses::{GetLabelsResponse, PatchLabelResponse, PostLabelsResponse, PutLabelResponse};

use mail_api_event_types::{Action, EventId};
use serde::Deserialize;
#[cfg(feature = "mocks")]
use serde::Serialize;
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{BoolFromInt, DefaultOnNull, serde_as};

mail_proton_ids::declare_proton_id! {
    pub LabelId
}

/// Represents which kind of label we are dealing with.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize_repr,
    Eq,
    Hash,
    PartialEq,
    Serialize_repr
)]
#[repr(u8)]
pub enum LabelType {
    Label = 1,
    ContactGroup = 2,
    Folder = 3,
    System = 4,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct Label {
    #[serde(rename = "ID")]
    pub id: LabelId,

    #[serde(rename = "ParentID")]
    pub parent_id: Option<LabelId>,

    pub color: String,

    #[serde_as(as = "DefaultOnNull<BoolFromInt>")]
    pub display: bool,

    #[serde_as(as = "DefaultOnNull<BoolFromInt>")]
    pub expanded: bool,

    #[serde(rename = "Type")]
    pub label_type: LabelType,

    pub name: String,

    #[serde_as(as = "DefaultOnNull<BoolFromInt>")]
    pub notify: bool,

    #[serde(default)]
    pub order: u32,

    pub path: Option<String>,

    #[serde_as(as = "DefaultOnNull<BoolFromInt>")]
    pub sticky: bool,

    #[serde(rename = "LastUnseenMessageEventID")]
    pub last_unseen_message: Option<EventId>,
}

#[cfg(feature = "mocks")]
impl Label {
    #[must_use]
    pub fn test_default() -> Self {
        Self {
            id: LabelId::from(""),
            parent_id: None,
            color: String::default(),
            display: false,
            expanded: false,
            label_type: LabelType::Label,
            name: String::default(),
            notify: false,
            order: 0,
            path: None,
            sticky: false,
            last_unseen_message: None,
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct LabelEvent {
    #[serde(rename = "ID")]
    pub id: LabelId,

    pub action: Action,

    pub label: Option<Label>,
}
