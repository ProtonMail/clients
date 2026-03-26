use mail_core_common::datatypes::LocalLabelId;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CategoryView {
    pub enabled: Option<LocalLabelId>,
    pub available: Vec<LocalLabelId>,
}
