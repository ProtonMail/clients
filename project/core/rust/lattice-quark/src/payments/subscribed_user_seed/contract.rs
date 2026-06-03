use lattice::LatticeError;

use crate::{LtQuarkContract, QuarkCommand};

use super::LtQuarkNewPaymentsSeedSubscribedUserRes;

/// Seed a user with a subscription in the new-payments system.
///
/// Quark CLI equivalent:
///
/// ```text
/// ./quark new-payments:seed:subscribed-user [options] [--] [<username> [<password> [<plan> [<cycle> [<currency>]]]]]
/// ```
pub struct LtQuarkNewPaymentsSeedSubscribedUser {
    pub username: String,
    pub password: String,
    /// Subscription plan payload (can be a simple string or JSON for multiple plans)
    /// Examples:
    /// - Simple: "mail2022"
    /// - Multiple: "{\"mail2022\": 1, \"1member-bundlepro2024\": 3}"
    pub plan: Option<String>,
    /// Plan cycle
    pub cycle: Option<String>,
    /// Currency code
    pub currency: Option<String>,
    /// Coupon to apply to the subscription (e.g., "SUPPORTER100")
    pub coupon: Option<String>,
    /// Create the subscription as a trial
    pub trial: bool,
    /// If set, the username is the id of the user
    pub username_is_id: bool,
}

impl Default for LtQuarkNewPaymentsSeedSubscribedUser {
    fn default() -> Self {
        Self {
            username: "subscriber-test".to_string(),
            password: "12341234".to_string(),
            plan: None,
            cycle: None,
            currency: None,
            coupon: None,
            trial: false,
            username_is_id: false,
        }
    }
}

impl LtQuarkContract for LtQuarkNewPaymentsSeedSubscribedUser {
    const COMMAND_PATH: &'static str = "new-payments:seed:subscribed-user";
    type Response = LtQuarkNewPaymentsSeedSubscribedUserRes;

    fn params(&self) -> Result<QuarkCommand, LatticeError> {
        let cmd = QuarkCommand::default()
            .query_if_some("--coupon", self.coupon.as_ref())
            .query_flag_if(self.trial, "--trial")
            .query_flag_if(self.username_is_id, "--username-is-id")
            .value(&self.username)
            .value(&self.password);

        if let Some(ref plan) = self.plan {
            let cmd = cmd.value(plan);
            if let Some(cycle) = self.cycle.clone() {
                let cmd = cmd.value(cycle);
                Ok(match &self.currency {
                    Some(c) => cmd.value(c),
                    None => cmd,
                })
            } else if self.currency.is_some() {
                Err(LatticeError::UnexpectedResponse(
                    "new-payments:seed:subscribed-user: `currency` requires `cycle` when `plan` is set"
                        .to_string(),
                ))
            } else {
                Ok(cmd)
            }
        } else {
            Ok(cmd)
        }
    }
}
