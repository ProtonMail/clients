use std::sync::LazyLock;

use regex::Regex;

use lattice::LatticeError;

use crate::LtQuarkRes;

static SEED_SUBSCRIBED_USER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"User `(.*)` \(ID (\d+)\) seeded correctly\.").expect("static regex")
});

// Plain-text log line, e.g. `User `name` (ID 101351) seeded correctly.`
#[derive(Debug, Clone)]
pub struct LtQuarkNewPaymentsSeedSubscribedUserRes {
    pub user_id: u64,
    pub username: String,
}

impl LtQuarkRes for LtQuarkNewPaymentsSeedSubscribedUserRes {
    fn from_quark_body(body: &[u8]) -> Result<Self, LatticeError> {
        let body_str: String = String::from_utf8(body.to_vec())
            .map_err(|e| LatticeError::UnexpectedResponse(e.to_string()))?;

        //  [INFO] User `ssoa_ylqeCkjm` (ID 101351) seeded correctly.
        let captures = SEED_SUBSCRIBED_USER_RE
            .captures(&body_str)
            .ok_or_else(|| LatticeError::UnexpectedResponse(body_str.to_string()))?;
        let username = captures
            .get(1)
            .ok_or_else(|| {
                LatticeError::UnexpectedResponse(
                    "seed subscribed-user: missing username capture".to_string(),
                )
            })?
            .as_str()
            .to_string();
        let user_id = captures
            .get(2)
            .ok_or_else(|| {
                LatticeError::UnexpectedResponse(
                    "seed subscribed-user: missing user id capture".to_string(),
                )
            })?
            .as_str()
            .parse::<u64>()
            .map_err(|e| {
                LatticeError::UnexpectedResponse(format!(
                    "seed subscribed-user: invalid user id: {e}"
                ))
            })?;

        Ok(LtQuarkNewPaymentsSeedSubscribedUserRes { user_id, username })
    }
}
