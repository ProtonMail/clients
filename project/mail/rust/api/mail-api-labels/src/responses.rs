//! Label API response structures.

use std::collections::HashMap;

use serde::Deserialize;
#[cfg(feature = "mocks")]
use serde::Serialize;
use serde::de::Deserializer;

use crate::Label;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct GetLabelsResponse {
    #[serde(deserialize_with = "deserialize_labels")]
    pub labels: Vec<Label>,
}

fn deserialize_labels<'de, D>(deserializer: D) -> Result<Vec<Label>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    pub enum LabelsMapOrList {
        Map(HashMap<String, Label>),
        List(Vec<Label>),
    }

    impl LabelsMapOrList {
        pub fn into_vec(self) -> Vec<Label> {
            match self {
                LabelsMapOrList::Map(map) => map.into_values().collect(),
                LabelsMapOrList::List(list) => list,
            }
        }
    }

    LabelsMapOrList::deserialize(deserializer).map(LabelsMapOrList::into_vec)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct PostLabelsResponse {
    pub label: Label,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct PutLabelResponse {
    pub label: Label,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(feature = "mocks", derive(Serialize))]
#[serde(rename_all = "PascalCase")]
pub struct PatchLabelResponse {
    pub label: Label,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LabelType;

    #[test]
    fn test_deserialize_labels_from_array() {
        let json = r##"{
            "Code": 1000,
            "Labels": [
                {
                    "ID": "sRNM_8TWzD4nSi55oC2B0-iV6avsMAAfDQZh7Bzsjy8c9Ip_c5OK5Tp5jB3mIEFmfUh3vFC9tevpCyXwoAa81w==",
                    "Name": "new 3",
                    "Path": "new 3",
                    "Type": 3,
                    "Color": "#415DF0",
                    "Order": 263,
                    "Notify": 1,
                    "Expanded": 0,
                    "Sticky": 0,
                    "Display": 1
                }
            ]
        }"##;

        let response: GetLabelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.labels.len(), 1);
        assert_eq!(
            response.labels[0].id.as_str(),
            "sRNM_8TWzD4nSi55oC2B0-iV6avsMAAfDQZh7Bzsjy8c9Ip_c5OK5Tp5jB3mIEFmfUh3vFC9tevpCyXwoAa81w=="
        );
        assert_eq!(response.labels[0].name, "new 3");
        assert_eq!(response.labels[0].path, Some("new 3".to_string()));
        assert_eq!(response.labels[0].label_type, LabelType::Folder);
        assert_eq!(response.labels[0].color, "#415DF0");
        assert_eq!(response.labels[0].order, 263);
    }

    #[test]
    fn test_deserialize_labels_from_map() {
        let json = r##"{
            "Code": 1000,
            "Labels": {
                "sRNM_8TWzD4nSi55oC2B0-iV6avsMAAfDQZh7Bzsjy8c9Ip_c5OK5Tp5jB3mIEFmfUh3vFC9tevpCyXwoAa81w==": {
                    "ID": "sRNM_8TWzD4nSi55oC2B0-iV6avsMAAfDQZh7Bzsjy8c9Ip_c5OK5Tp5jB3mIEFmfUh3vFC9tevpCyXwoAa81w==",
                    "Name": "new 3",
                    "Path": "new 3",
                    "Type": 3,
                    "Color": "#415DF0",
                    "Order": 263,
                    "Notify": 1,
                    "Expanded": 0,
                    "Sticky": 0,
                    "Display": 1
                }
            }
        }"##;

        let response: GetLabelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.labels.len(), 1);
        assert_eq!(
            response.labels[0].id.as_str(),
            "sRNM_8TWzD4nSi55oC2B0-iV6avsMAAfDQZh7Bzsjy8c9Ip_c5OK5Tp5jB3mIEFmfUh3vFC9tevpCyXwoAa81w=="
        );
        assert_eq!(response.labels[0].name, "new 3");
        assert_eq!(response.labels[0].path, Some("new 3".to_string()));
        assert_eq!(response.labels[0].label_type, LabelType::Folder);
        assert_eq!(response.labels[0].color, "#415DF0");
        assert_eq!(response.labels[0].order, 263);
    }
}
