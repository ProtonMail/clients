use mail_api::services::proton::common::{AttachmentId, ConversationId, MessageId};
use mail_core_api::services::proton::IncomingDefaultId;
use mail_core_common::declare_local_id;

declare_local_id!(pub LocalAttachmentId => AttachmentId);
declare_local_id!(pub LocalMessageId => MessageId);
declare_local_id!(pub LocalConversationId => ConversationId);
declare_local_id!(pub LocalIncomingDefaultId => IncomingDefaultId);
