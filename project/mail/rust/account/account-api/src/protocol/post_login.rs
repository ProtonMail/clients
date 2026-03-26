//! Post-login validation types.

use async_trait::async_trait;
use mail_observability::metric;
use serde::{Deserialize, Serialize};

/// This enum defines possible error conditions encountered after a successful login,
/// focusing on constraints and limits that might prevent further actions.
#[derive(Debug, thiserror::Error)]
pub enum PostLoginValidationError {
    /// Indicates that the maximum number of free accounts has been exceeded. Contains the max number of free accounts allowed.
    #[error("The maximum number of free accounts has been exceeded.")]
    FreeAccountLimitExceeded(u64),

    #[error("Error during post login check: {0}")]
    Other(#[from] anyhow::Error),
}

/// Trait for validating a user after login.
///
/// Implementations can perform additional checks after a user has been authenticated,
/// such as verifying free account limits.
#[async_trait]
pub trait PostLoginValidator: Send + Sync {
    async fn validate(
        &self,
        user: &crate::protocol::proton::User,
    ) -> Result<(), PostLoginValidationError>;
}

metric! {
    #[name = "core_signup_user_check_total"]
    #[version = 1]
    #[doc = "Records the outcomes of the post login user checks."]
    pub struct UserCheckResult {
        pub status: UserCheckStatus,
    }
}

#[derive(PartialEq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserCheckStatus {
    Success,
    Failure,
}
