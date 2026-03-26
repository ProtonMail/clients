mod responses;

pub use self::responses::*;

/// The Proton Auth API base path (v4).
pub const AUTH_V4: &str = "/auth/v4";

pub use mail_account_api::protocol::proton::{
    PostAuthInfoRequest, PostAuthInfoResponse, ProtonAuth,
};
