use crate::Mailbox;
use mail_core_common::models::Label;
use mail_stash::orm::Model;
use mail_stash::stash::Tether;

#[allow(async_fn_in_trait)]
pub trait MailboxTestUtils {
    async fn get_local_label(&self, mail_stash: &Tether) -> Label;
}

impl MailboxTestUtils for Mailbox {
    async fn get_local_label(&self, tether: &Tether) -> Label {
        Label::load(self.label_id(), tether).await.unwrap().unwrap()
    }
}
