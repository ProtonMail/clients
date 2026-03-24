use mail_proton_ids::declare_proton_id;

declare_proton_id! {
    pub UserId
}

impl UserId {
    #[must_use]
    pub fn short_id(&self) -> String {
        self.0[..10].to_string()
    }
}

declare_proton_id! {
    pub SessionId
}
