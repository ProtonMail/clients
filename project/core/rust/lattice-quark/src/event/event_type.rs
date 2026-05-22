#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum LtQuarkEventType {
    User = 5,
    Addr = 13,
    UserSettings = 30,
    MailSettings = 31,
}

impl std::fmt::Display for LtQuarkEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u8)
    }
}
