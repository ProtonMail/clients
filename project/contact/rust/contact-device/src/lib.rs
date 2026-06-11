use mail_proton_ids::PrivateEmail;

/// A contact as read straight from the device's native address book,
/// held only in memory and **never persisted**.
///
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceContact {
    /// Platform's own opaque identifier for the entry.
    pub id: String,
    pub display_name: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub emails: Vec<PrivateEmail>,
    pub phones: Vec<String>,
}
