#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum LtQuarkEventAction {
    Delete = 0,
    Create = 1,
    Update = 2,
    UpdateFlags = 3,
}

impl std::fmt::Display for LtQuarkEventAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u8)
    }
}
