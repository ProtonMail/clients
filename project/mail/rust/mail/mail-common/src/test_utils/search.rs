use crate::datatypes::SystemLabelId;
use mail_api::services::proton::common::{AttachmentId, ConversationId};
use mail_api::services::proton::response_data::{
    AttachmentMetadata, Conversation as ApiConversation, ConversationLabel as ApiConversationLabel,
    MessageRecipient as ApiMessageRecipient, MessageSender as ApiMessageSender,
};
use mail_core_api::services::proton::LabelId;
use mail_core_common::datatypes::{LabelColor, LabelType, LocalLabelId};
use mail_core_common::models::Label;
use mail_core_common::models::ModelIdExtension;
use mail_stash::orm::Model;
use mail_stash::stash::{StashError, Tether};
use std::collections::BTreeMap;
use std::sync::LazyLock;

pub static MY_LABEL_ID1: LazyLock<LabelId> = LazyLock::new(|| LabelId::from("MyLabelID1"));
pub static MY_LABEL_ID2: LazyLock<LabelId> = LazyLock::new(|| LabelId::from("MyLabelID2"));

pub static MY_ATTACHMENT_ID: LazyLock<AttachmentId> =
    LazyLock::new(|| AttachmentId::from("MyAttachmentID1"));

pub static MY_CONVERSATION_ID: LazyLock<ConversationId> =
    LazyLock::new(|| ConversationId::from("MyConversationID"));

#[macro_export]
macro_rules! conv_id {
    ($id:expr) => {{
        use mail_api::services::proton::common::ConversationId;
        Some(ConversationId::from($id.to_string()).into())
    }};
}

#[macro_export]
macro_rules! lbl_id {
    ($id:expr) => {{
        use mail_core_api::services::proton::LabelId;
        Some(LabelId::from($id.to_string()).into())
    }};
}

#[macro_export]
macro_rules! msg_id {
    ($id:expr) => {{
        use mail_api::services::proton::common::MessageId;
        Some(MessageId::from($id.to_string()).into())
    }};
}

#[macro_export]
macro_rules! label {
    ($($field:tt)*) => {{
        use mail_core_common::models::Label;

        Label {
            $($field)*,
            ..Label::test_default()
        }}
    };
}

#[macro_export]
macro_rules! api_label {
    ($($field:tt)*) => {{
        use mail_core_api::services::proton::{Label as ApiLabel};

        ApiLabel {
            $($field)*,
            ..ApiLabel::test_default()
        }
    }};
}

#[macro_export]
macro_rules! message {
    ($($field:tt)*) => {{
        use $crate::models::Message;

        Message {
            $($field)*,
            ..Message::test_default()
        }
    }};
}

#[macro_export]
macro_rules! api_message {
    ($($field:tt)*) => {{
        use mail_api::services::proton::response_data::{Message as ApiMessage};

        ApiMessage {
            $($field)*,
            ..Default::default()
        }
    }};
}

#[macro_export]
macro_rules! api_message_meta {
    ($($field:tt)*) => {{
        use mail_api::services::proton::response_data::{MessageMetadata as ApiMessageMetadata};

        ApiMessageMetadata {
            $($field)*,
            ..ApiMessageMetadata::test_default()
        }
    }};
}

#[macro_export]
macro_rules! conversation {
    ($($field:tt)*) => {{
        use $crate::models::Conversation;

        Conversation {
            $($field)*,
            ..Conversation::test_default()
        }
    }};
}

#[macro_export]
macro_rules! conv_label {
    ($($field:tt)*) => {{
        use $crate::models::ConversationLabel;

        ConversationLabel {
            $($field)*,
            ..ConversationLabel::test_default()
        }
    }};
}

#[macro_export]
macro_rules! api_conversation {
    ($($field:tt)*) => {{
        use mail_api::services::proton::response_data::{Conversation as ApiConversation};

        ApiConversation {
            $($field)*,
            ..ApiConversation::test_default()
        }
    }};
}

/// Can panic if the local conversation `id` is not set, the remote
/// `label_id` is not set, the local `label` can not be found or the query
/// failed.
///
pub async fn create_labels(tether: &mut Tether) -> Vec<LocalLabelId> {
    let mut labels = [test_label1(), test_label2()];
    tether
        .write_tx::<_, _, StashError>(async |tx| {
            for label in &mut labels {
                label.save(tx).await.expect("failed to create labels");
                assert!(
                    Label::find_by_remote_id(label.remote_id.clone().unwrap(), tx)
                        .await
                        .expect("failed to resolve label ids")
                        .unwrap()
                        .local_id
                        .is_some()
                );
            }
            Ok(())
        })
        .await
        .expect("failed to commit transaction");

    labels.into_iter().map(|l| l.id()).collect()
}

#[must_use]
pub fn test_label1() -> Label {
    label!(
        remote_id: Some(MY_LABEL_ID1.clone()),
        name: "MyLabel".to_owned(),
        color: LabelColor::black(),
        label_type: LabelType::Label
    )
}

#[must_use]
pub fn test_label2() -> Label {
    label!(
       remote_id: Some(MY_LABEL_ID2.clone()),
       name: "MyFolder".to_owned(),
       color: LabelColor::black(),
       label_type: LabelType::Folder,
       notify: true,
       expanded: true,
       display_order: 1
    )
}

#[must_use]
pub fn test_starred_label() -> Label {
    label!(
       remote_id: Some(LabelId::starred().clone()),
       name: "Starred".to_owned(),
       path: Some("Starred".to_owned()),
       color: LabelColor::black(),
       label_type: LabelType::System,
       display_order: 2
    )
}

pub fn test_conversation(
    labels: impl IntoIterator<Item = ApiConversationLabel>,
    attachments: impl IntoIterator<Item = AttachmentMetadata>,
) -> ApiConversation {
    ApiConversation {
        id: MY_CONVERSATION_ID.clone(),
        order: 50,
        subject: "Hello World".to_owned(),
        senders: vec![ApiMessageSender {
            address: "hello@world.com".into(),
            name: "HelloWorld".into(),
            ..Default::default()
        }],
        recipients: vec![
            ApiMessageRecipient {
                address: "foo@bar.com".into(),
                name: "Foo".into(),
                ..Default::default()
            },
            ApiMessageRecipient {
                address: "Bar@bar.com".into(),
                name: "bar".into(),
                ..Default::default()
            },
        ],
        num_messages: 10,
        num_unread: 4,
        num_attachments: 7,
        expiration_time: 1024,
        size: 4909,
        labels: Vec::from_iter(labels),
        display_snoozed_reminder: false,
        attachments_metadata: Vec::from_iter(attachments),
        attachment_info: BTreeMap::default(),
        context_time: None,
    }
}
