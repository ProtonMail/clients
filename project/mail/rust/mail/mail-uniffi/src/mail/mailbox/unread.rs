#[uniffi::export(callback_interface)]
pub trait UnreadLiveQueryCallback: Send + Sync {
    fn on_update(&self, unread: u64);
}
