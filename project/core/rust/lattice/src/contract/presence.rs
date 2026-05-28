//! SlimAPI presence-only query flags (wire value is empty), e.g. `Handles`.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LtSlimApiPresenceQuery;

impl serde::Serialize for LtSlimApiPresenceQuery {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("")
    }
}
