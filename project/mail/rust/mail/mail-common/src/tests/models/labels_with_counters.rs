use crate::datatypes::{ConversationLabelsCount, MessageLabelsCount};
use crate::models::LabelWithCounters;
use mail_common::test_utils::db::new_test_connection;
use mail_core_api::services::proton::{Label as ApiLabel, LabelId, LabelType as ApiLabelType};
use mail_core_common::models::Label;
use mail_stash::orm::Model;
use mail_stash::stash::StashError;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn label_with_counters() {
    let mut tether = new_test_connection().await.connection();
    let label = ApiLabel {
        id: LabelId::from("label"),
        parent_id: None,
        name: "Label".to_owned(),
        path: None,
        color: "00".to_owned(),
        label_type: ApiLabelType::Label,
        notify: false,
        display: false,
        sticky: false,
        expanded: false,
        order: 0,
    };

    let total_conv = 20_u64;
    let unread_conv = 40_u64;
    let total_msg = 200_u64;
    let unread_msg = 600_u64;

    let mut local_label = Label::from(label.clone());

    let local_id = tether
        .write_tx::<_, _, StashError>(async |tx| {
            local_label.save(tx).await.unwrap();

            ConversationLabelsCount::upsert(
                vec![ConversationLabelsCount {
                    label_id: local_label.remote_id.clone().unwrap(),
                    total: total_conv,
                    unread: unread_conv,
                }],
                tx,
            )
            .await
            .unwrap();

            MessageLabelsCount::upsert(
                vec![MessageLabelsCount {
                    label_id: local_label.remote_id.clone().unwrap(),
                    total: total_msg,
                    unread: unread_msg,
                }],
                tx,
            )
            .await
            .unwrap();

            Ok(local_label.id())
        })
        .await
        .unwrap();

    let counters = LabelWithCounters::load(local_id, &tether)
        .await
        .expect("failed to load counter")
        .expect("should have a value");

    assert_eq!(counters.unread_conv, unread_conv);
    assert_eq!(counters.total_conv, total_conv);
    assert_eq!(counters.unread_msg, unread_msg);
    assert_eq!(counters.total_msg, total_msg);
}
