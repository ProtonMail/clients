use std::str::FromStr;

use crate::{
    LatticeError,
    quark::{LtQuarkContract, LtQuarkResTryFrom, QuarkCommand},
};

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

/// Add an event to the quark system state
/// Equivalent of ./quark event:add --uid <session_id> -- <username> <event_type> <item_id> <event_action>
pub struct LtQuarkEventAdd {
    pub username: String,
    pub session_id: String,
    pub event_type: LtQuarkEventType,
    pub item_id: u64,
    pub event_action: LtQuarkEventAction,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LtQuarkEventAddResponse {
    Success,
}

impl FromStr for LtQuarkEventAddResponse {
    type Err = LatticeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s == "Event added successfully" {
            return Ok(LtQuarkEventAddResponse::Success);
        }
        Err(LatticeError::UnexpectedResponse(s.to_string()))
    }
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
