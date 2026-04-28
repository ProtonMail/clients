//! Member **auth device** routes: `/core/v4/members/.../devices...`.
//!
//! The `core` crate feature depends on `auth` ([`crate::auth`]), so list payloads reuse
//! [`LtAuthDevice`] (same wire shape as `GET /auth/v4/devices`). Those types are re-exported here for
//! discoverability; [`crate::auth::devices`] is the canonical path.

mod delete_bulk;
mod delete_one;
mod get_list;
mod get_pending;
mod post_reset;
mod put_reject;

pub use crate::auth::devices::{LtAuthDevice, LtAuthDeviceState};
pub use delete_bulk::LtCoreDeleteMembersDevicesReq;
pub use delete_one::LtCoreDeleteMembersDeviceReq;
pub use get_list::{LtCoreGetMembersDevicesReq, LtCoreGetMembersDevicesRes};
pub use get_pending::{LtCoreGetMembersDevicesPendingReq, LtCoreGetMembersDevicesPendingRes};
pub use post_reset::{
    LtCorePostMembersDevicesResetBody, LtCorePostMembersDevicesResetReq,
    LtCoreResetAuthDevicesUserKey,
};
pub use put_reject::LtCorePutMembersDevicesRejectReq;
