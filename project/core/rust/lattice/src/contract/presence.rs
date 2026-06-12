//! SlimAPI presence-only query flags (wire value is empty), e.g. `Handles`.

use serde::{Serialize, Serializer};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LtSlimApiPresenceQuery;

impl Serialize for LtSlimApiPresenceQuery {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("")
    }
}
