//! Protocol types for the account API.

pub mod device;
pub mod migration;
pub mod observability;
pub mod passphrase;
pub mod post_login;
pub mod proton;
mod proton_impl;

pub use device::{DeviceInfo, DeviceInfoProvider, DynDeviceInfoProvider};
pub use migration::{MigrationSnooper, NoopMigrationSnooper};
pub use observability::ApiServiceObservabilityResponse;
pub use passphrase::{PassphraseAcquireError, PassphraseProvider};
pub use post_login::{
    PostLoginValidationError, PostLoginValidator, UserCheckResult, UserCheckStatus,
};
