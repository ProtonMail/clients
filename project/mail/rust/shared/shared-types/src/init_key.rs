/// Key used to distinguish between components in the initialization.
/// It is a string, not an enum for making it open for additional changes from different BU.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InitializationKey(pub &'static str);

impl InitializationKey {
    #[must_use]
    pub const fn new(s: &'static str) -> Self {
        Self(s)
    }
}

impl From<InitializationKey> for String {
    fn from(value: InitializationKey) -> Self {
        value.0.to_owned()
    }
}
