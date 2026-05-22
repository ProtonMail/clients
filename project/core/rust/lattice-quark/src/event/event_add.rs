use lattice::LatticeError;

use crate::{LtQuarkContract, LtQuarkResTryFrom, QuarkCommand};

use super::{LtQuarkEventAction, LtQuarkEventAddResponse, LtQuarkEventType};

/// Add an event to the quark system state
/// Equivalent of ./quark event:add --uid <session_id> -- <username> <event_type> <item_id> <event_action>
pub struct LtQuarkEventAdd {
    pub username: String,
    pub session_id: String,
    pub event_type: LtQuarkEventType,
    pub item_id: u64,
    pub event_action: LtQuarkEventAction,
}

impl LtQuarkContract for LtQuarkEventAdd {
    const COMMAND_PATH: &'static str = "event:add";
    type Response = LtQuarkResTryFrom<LtQuarkEventAddResponse>;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        Ok(QuarkCommand::default()
            .query("--uid", &self.session_id)
            .value(&self.username)
            .value(self.event_type)
            .value(self.item_id)
            .value(self.event_action))
    }
}
